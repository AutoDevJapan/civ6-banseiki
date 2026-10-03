-- Test-only UI observer and real-frame requests. No Gameplay property writes.
include("JAC_PersonaBoundaryRules");
local R = JAC_PersonaBoundaryRules;
local B = {};
ExposedMembers.JAC_PersonaBoundary = B;
local RETRY = "JAC_BOUNDARY_ENEMY_SPY_RETRY";
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
local captiveExchange;
local emptyRosterObservation;

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

local function CheckArchivedMission(c)
    local retry=Game:GetProperty(RETRY);
    if not retry then return nil; end
    local a=assert(retry.archive,"retry has no archived native failure");
    assert(retry.current==2 and a.eventID>=0 and a.spyID>=0 and
        a.targetOwner==c.owner and a.targetID==c.cityID and
        a.governorNeutralized==0,"archived attempt identity changed");
    local diplomacy=Players[c.attacker]:GetDiplomacy();
    local native=diplomacy:GetMission(c.attacker,a.eventID);
    assert(type(native)=="table" and native.Operation==a.operation and
        native.PlotIndex==a.plot and native.CompletionTurn==a.completionTurn and
        native.InitialResult==a.initial and native.EscapeResult==a.escape and
        a.initial~=EspionageResultTypes.SUCCESS_UNDETECTED and
        a.initial~=EspionageResultTypes.SUCCESS_MUST_ESCAPE,
        "archived failure differs from preserved native mission history");
    return retry;
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
    s.retry=CheckArchivedMission(c);
    return s;
end
function B.ReadEnemyResultDiagnostic()
    local c=EnemyConfig();
    local snapshot=B.ReadEnemy();
    local event=Game:GetProperty("JAC_BOUNDARY_ENEMY_MISSION");
    local mission=event and snapshot.missions[c.attacker.."/"..event.id];
    Log("ENEMY_FAILED_MISSION",{turn=snapshot.turn,event=event,
        mission=mission,governor=snapshot.native,spy=snapshot.spy,
        roster=snapshot.spyRoster,
        capacity=Players[c.attacker]:GetDiplomacy():GetSpyCapacity()});
    return B.EnemyProgress();
end
function B.CheckEnemy(phase, merit)
    local s=B.ReadEnemy(); Log("ENEMY_CHECK_"..phase,s);
    assert(s.native.turnsToEstablish==5,"native five-turn establishment duration changed");
    R.Check(s,phase,merit); return R.Serialize(s);
end
function B.RecordEnemy(label)
    local s=B.ReadEnemy();
    Log(label.."_MISSION",{turn=s.turn,missions=s.missions,success=s.neutralizationMission,
        native=s.native,spy=s.spy});
    Log(label.."_ROSTER",s.spyRoster or {});
    return Log(label, s);
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
    local s=B.EnemyState();
    if c.cityID then
        assert(s.appointed and s.base and s.personOwner==c.owner and
            s.personaState=="GOVERNOR" and s.nativeType==GameInfo.Governors[
                GameInfo.JAC_LeaderPersonas[c.leader].GovernorType].Index and
            s.total>=c.total and s.spent==c.spent+1 and
            s.cityOwner==c.owner and s.cityID==c.cityID and
            s.cityX==c.x and s.cityY==c.y,
            "saved enemy appointment no longer matches the native Governor");
        B.lastAction="ALREADY_CONFIRMED_ENEMY";
        return "NATIVE_BOUND";
    end
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
    Log("ENEMY_SPY_ARRIVAL_ROSTER",{turn=Game.GetCurrentGameTurn(),receipt=receipt,roster=roster});
    if #roster.activeOffMap==1 and #roster.map==0 then return "TRAVEL"; end
    if #roster.activeOffMap==0 and #roster.map==0 then return "ARRIVAL_UNOBSERVED"; end
    assert(#roster.activeOffMap==0 and #roster.map==1,
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
    local roster={map={},offMap={},captured={},activeOffMap={}};
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
    -- EspionageOverview.lua enumerates every captor's roster, then filters
    -- OwningPlayer. A captive occupies the native operative count.
    for _,captor in ipairs(Game.GetPlayers()) do
        local captorID=captor:GetID();
        local diplo=captor:GetDiplomacy();
        local held=diplo:GetNumSpiesCaptured();
        assert(type(held)=="number" and held>=0 and held<=256,
            "native captive spy count unavailable");
        for i=0,held-1 do
            local captive=assert(diplo:GetNthCapturedSpy(captorID,i),
                "native captive spy record missing");
            assert(type(captive.OwningPlayer)=="number",
                "native captive owner unavailable");
            if captive.OwningPlayer==owner then
                assert(type(captive.Name)=="string" and type(captive.Level)=="number" and
                    type(captive.NameIndex)=="number",
                    "native captive spy fields unavailable");
                roster.captured[#roster.captured+1]={captor=captorID,name=captive.Name,
                    level=captive.Level,nameIndex=captive.NameIndex};
            end
        end
    end
    -- Live 86 exposed the same captive once in the captor's roster and once
    -- as an off-map sentinel. Retain both raw records, and discount only the
    -- unique exact sentinel belonging to the unique captive. Real travel has
    -- a future ReturnTurn and must always count as another operative.
    if #roster.captured==1 then
        local captive=roster.captured[1];
        local defaultName=GameInfo.Units["UNIT_SPY"].Name;
        local matchIndex, matches=nil,0;
        for i,record in ipairs(roster.offMap) do
            local namesMatch=record.name==captive.name or
                (record.name=="" and captive.name==defaultName) or
                (captive.name=="" and record.name==defaultName);
            if record.returnTurn==-1 and record.x==-9999 and record.y==-9999 and
                namesMatch and record.level==captive.level then
                matchIndex=i; matches=matches+1;
            end
        end
        if matches==1 then
            roster.capturedPlaceholder={offMapIndex=matchIndex,captiveIndex=1};
        end
    end
    for i,record in ipairs(roster.offMap) do
        if not roster.capturedPlaceholder or roster.capturedPlaceholder.offMapIndex~=i then
            roster.activeOffMap[#roster.activeOffMap+1]=record;
        end
    end
    roster.physicalCount=#roster.map+#roster.activeOffMap+#roster.captured;
    if owner==Game.GetLocalPlayer() and
        (#roster.map>0 or #roster.offMap>0 or #roster.captured>0) then
        -- Any complete nonempty roster snapshot breaks the continuous empty window.
        emptyRosterObservation=nil;
    end
    return roster;
end

InTargetCity=function(x,y,owner,id)
    local plot=Map.GetPlot(x,y);
    local city=plot and Cities.GetPlotPurchaseCity(plot);
    return city and city:GetOwner()==owner and city:GetID()==id or false;
end

function B.EnemyRetryDiagnosis()
    local c=EnemyConfig();
    assert(not Game:GetProperty(RETRY),"finite enemy retry already archived");
    local s=B.ReadEnemy();
    local event=assert(Game:GetProperty("JAC_BOUNDARY_ENEMY_MISSION"),
        "preserved failed native mission event missing");
    local reference=assert(Game:GetProperty("JAC_BOUNDARY_SPY"),
        "preserved failed native spy reference missing");
    local travel=assert(Game:GetProperty("JAC_BOUNDARY_SPY_TRAVEL"),
        "original accepted spy travel missing");
    local mission=assert(s.missions[c.attacker.."/"..event.id],
        "failed event has no matching native mission history");
    assert(event.attacker==c.attacker and reference.owner==c.attacker and
        s.spy and s.spy.id==reference.id and
        travel.targetOwner==c.owner and travel.targetID==c.cityID and
        mission.turn==event.turn and
        mission.initial~=EspionageResultTypes.SUCCESS_UNDETECTED and
        mission.initial~=EspionageResultTypes.SUCCESS_MUST_ESCAPE and
        not s.neutralizationMission and s.native.neutralized==0,
        "retry requires the actual failed mission with no native neutralization");
    R.Check(s,"SETTLED");
    local roster=assert(s.spyRoster,"native spy roster missing");
    local occupied=roster.physicalCount;
    local capacity=Players[c.attacker]:GetDiplomacy():GetSpyCapacity();
    assert(type(capacity)=="number" and capacity>=0,
        "native spy capacity unavailable");
    local observationKey=R.Serialize({attacker=c.attacker,defender=c.owner,
        source=c.source,leader=c.leader,cityID=c.cityID,x=c.x,y=c.y,
        event=event,spy=reference,travel=travel,mission=mission});
    if emptyRosterObservation and emptyRosterObservation.key~=observationKey then
        emptyRosterObservation=nil;
    end
    assert(#roster.map<=1 and #roster.activeOffMap<=1 and #roster.captured<=1 and
        occupied<=1,"retry requires an unambiguous original operative roster");
    local result={spyID=reference.id,eventID=event.id,eventTurn=event.turn,
        mission=mission,roster=roster,capacity=capacity,occupied=occupied,
        spent=s.titles.spent,total=s.titles.total,governorType=s.native.type};
    if #roster.captured==1 then
        local captive=roster.captured[1];
        local defaultName=GameInfo.Units["UNIT_SPY"].Name;
        assert(not s.spy.present and #roster.map==0 and #roster.activeOffMap==0 and
            captive.level==travel.level and
            (captive.name==travel.name or
             (captive.name=="" and captive.nameIndex==-1 and travel.name==defaultName)),
            "captured operative differs from the original spy");
        result.outcome="CAPTURED"; result.captive=captive;
        if occupied>=capacity then
            result.mode="UNAVAILABLE_CAPACITY";
            result.next="native captive exchange or capacity increase, then reread roster";
        else
            result.mode="PRODUCE";
        end
    elseif #roster.map==1 then
        local unit=roster.map[1];
        assert(unit.name==travel.name and unit.level==travel.level,
            "living map spy does not match the original operative");
        result.mode="MAP"; result.outcome="LIVING_MAP"; result.currentID=unit.id;
    elseif #roster.activeOffMap==1 then
        local unit=roster.activeOffMap[1];
        assert((unit.name==travel.name or
            (unit.name=="" and travel.name==GameInfo.Units["UNIT_SPY"].Name)) and
            unit.level==travel.level and unit.returnTurn>=s.turn,
            "off-map native spy differs from the original operative");
        result.mode="RETURN"; result.outcome="LIVING_OFF_MAP";
        result.returnName=unit.name; result.returnLevel=unit.level;
        result.returnTurn=unit.returnTurn; result.returnX=unit.x; result.returnY=unit.y;
    else
        assert(not s.spy.present,"spy reference is still present despite an empty map roster");
        -- Arrival may temporarily expose neither a map unit nor an off-map
        -- record. Require 60 seconds continuously empty in this UI load and
        -- failed-attempt identity before using native capacity to build.
        local now=UI.GetElapsedTime();
        assert(type(now)=="number" and now==now and now>=0 and now<math.huge,
            "UI observation clock unavailable");
        if not emptyRosterObservation or now<emptyRosterObservation.last then
            emptyRosterObservation={key=observationKey,since=now,last=now};
        else
            emptyRosterObservation.last=now;
        end
        if now-emptyRosterObservation.since<60 then
            result.mode="OBSERVATION_PENDING";
            result.outcome="NO_ROSTER_RECORD_YET";
            Log("ENEMY_SPY_RETRY_DIAGNOSIS",result);
            return result;
        end
        result.outcome="GONE_NO_CAPTURE_RECORD";
        if occupied>=capacity then
            result.mode="UNAVAILABLE_CAPACITY";
            result.next="obtain native spy capacity, then reread roster";
        else
            result.mode="PRODUCE";
        end
    end
    Log("ENEMY_SPY_RETRY_DIAGNOSIS",result);
    return result;
end

-- Stock DiplomacyDealView uses GetItemCount/Items and item IDs to inspect or
-- remove individual working-deal entries. Never clear the whole outgoing deal:
-- HasPendingDeal does not cover a player's unsent offer.
local function WorkingDealItemIDs(deal)
    local count=deal:GetItemCount();
    assert(type(count)=="number" and count>=0 and count<=256 and count%1==0,
        "native working deal item count unavailable");
    local ids={};
    for item in deal:Items() do
        assert(#ids<count,"native working deal enumeration exceeds item count");
        local id=item:GetID();
        assert(type(id)=="number" and id>=0,"native working deal item ID unavailable");
        ids[#ids+1]=id;
    end
    assert(#ids==count,"native working deal item enumeration differs from count");
    return ids;
end

-- Probe only a proven-empty native outgoing deal and remove only the item
-- created here. No session or offer is submitted; the engine decides whether
-- the actual negative captive NameIndex is valid.
function B.ProbeCapturedExchange()
    local c=EnemyConfig();
    local d=B.EnemyRetryDiagnosis();
    assert(d.outcome=="CAPTURED" and d.captive and d.occupied==1,
        "exchange probe requires the unique original captive");
    local captor=d.captive.captor;
    assert(captor~=c.attacker and type(d.captive.nameIndex)=="number",
        "native captive trade identity unavailable");
    local manager=assert(DealManager,"native deal manager unavailable");
    local function Report(status,extra)
        local result={status=status,captor=captor,nameIndex=d.captive.nameIndex,
            capacity=d.capacity,occupied=d.occupied};
        if extra then result.detail=extra; end
        Log("ENEMY_CAPTIVE_EXCHANGE_PROBE",{result=result,roster=d.roster});
        return result;
    end
    if Players[c.attacker]:GetDiplomacy():IsAtWarWith(captor) then
        return Report("AT_WAR");
    end
    if manager.HasPendingDeal(c.attacker,captor) then
        return Report("PENDING_DEAL");
    end
    local direction=DealDirection.OUTGOING;
    local readOK,deal=pcall(manager.GetWorkingDeal,direction,c.attacker,captor);
    if not readOK or not deal then return Report("WORKING_DEAL_UNAVAILABLE"); end
    local itemsOK,ids=pcall(WorkingDealItemIDs,deal);
    if not itemsOK then return Report("WORKING_DEAL_UNREADABLE",tostring(ids)); end
    if #ids~=0 then return Report("WORKING_DEAL_OCCUPIED",#ids); end
    if type(deal.FindItemByID)~="function" or type(deal.RemoveItemByID)~="function" then
        return Report("WORKING_DEAL_UNREADABLE","native item removal unavailable");
    end
    -- Repeat the read directly before AddItemOfType. If another UI context
    -- inserted an item, this invocation leaves that offer untouched.
    readOK,deal=pcall(manager.GetWorkingDeal,direction,c.attacker,captor);
    if not readOK or not deal then return Report("WORKING_DEAL_CHANGED"); end
    itemsOK,ids=pcall(WorkingDealItemIDs,deal);
    if not itemsOK or #ids~=0 then return Report("WORKING_DEAL_CHANGED"); end
    local created,createdID;
    local ok,status=pcall(function()
        created=deal:AddItemOfType(DealItemTypes.CAPTIVE,captor);
        if not created then return "NO_ITEM"; end
        createdID=created:GetID();
        assert(type(createdID)=="number" and createdID>=0,
            "native captive deal item ID unavailable");
        created:SetValueType(d.captive.nameIndex);
        return created:IsValid() and "VALID_ITEM" or "INVALID_ITEM";
    end);
    if not created then
        local afterOK,after=pcall(manager.GetWorkingDeal,direction,c.attacker,captor);
        if not afterOK or not after then return Report("WORKING_DEAL_CHANGED"); end
        local emptyOK,afterIDs=pcall(WorkingDealItemIDs,after);
        if not emptyOK or #afterIDs~=0 then return Report("WORKING_DEAL_CHANGED"); end
        if not ok then error(status); end
        return Report(status);
    end
    local cleanOK,cleanStatus=pcall(function()
        assert(createdID,"native captive deal item identity unavailable");
        local current=assert(manager.GetWorkingDeal(direction,c.attacker,captor),
            "native working deal disappeared before item cleanup");
        local beforeIDs=WorkingDealItemIDs(current);
        local otherIDs,ours={},0;
        for _,id in ipairs(beforeIDs) do
            if id==createdID then ours=ours+1;
            else otherIDs[#otherIDs+1]=id; end
        end
        assert(ours==1,"native probe item is no longer unique");
        local item=assert(current:FindItemByID(createdID),
            "native probe item disappeared before cleanup");
        assert(item==created and item:GetID()==createdID and
            item:GetType()==DealItemTypes.CAPTIVE and
            item:GetFromPlayerID()==captor and item:GetToPlayerID()==c.attacker and
            (not ok or item:GetValueType()==d.captive.nameIndex) and
            not item:IsLocked(),
            "native probe item changed before cleanup");
        assert(current:RemoveItemByID(createdID),"native probe item removal failed");
        local remaining=manager.GetWorkingDeal(direction,c.attacker,captor);
        local remainingIDs=remaining and WorkingDealItemIDs(remaining) or {};
        assert(#remainingIDs==#otherIDs,"unrelated working deal item count changed");
        for i,id in ipairs(otherIDs) do
            assert(remainingIDs[i]==id,"unrelated working deal item order changed");
        end
        return #otherIDs==0 and status or "WORKING_DEAL_CHANGED";
    end);
    if not cleanOK then error("native probe item cleanup unconfirmed: "..tostring(cleanStatus)); end
    if not ok then error(status); end
    return Report(cleanStatus);
end

local function ExactCaptiveDealItem(deal,captor,attacker,nameIndex)
    local ids=WorkingDealItemIDs(deal);
    if #ids~=1 then return nil; end
    local item=deal:FindItemByID(ids[1]);
    if item and item:GetID()==ids[1] and item:GetType()==DealItemTypes.CAPTIVE and
        item:GetFromPlayerID()==captor and item:GetToPlayerID()==attacker and
        item:GetValueType()==nameIndex then return item; end
    return nil;
end

local function RemovePreparedCaptiveItem(item,captor,attacker)
    local current=assert(DealManager.GetWorkingDeal(DealDirection.OUTGOING,attacker,captor),
        "native outgoing captive deal disappeared");
    local ids=WorkingDealItemIDs(current);
    local matches=0;
    for _,id in ipairs(ids) do if id==item:GetID() then matches=matches+1; end end
    assert(matches==1,"native prepared captive item ID is no longer unique");
    local candidate=assert(current:FindItemByID(item:GetID()),
        "native prepared captive item disappeared");
    assert(candidate:GetID()==item:GetID() and
        candidate:GetType()==DealItemTypes.CAPTIVE and
        candidate:GetFromPlayerID()==captor and candidate:GetToPlayerID()==attacker and
        candidate:GetValueType()==item:GetValueType(),
        "native prepared captive item changed");
    if candidate:IsLocked() then candidate:SetLocked(false); end
    assert(current:RemoveItemByID(item:GetID()),"native prepared captive item removal failed");
end

-- Stock EspionageOverview prepares and locks a captive item before opening
-- MAKE_DEAL. The stock deal view sends PROPOSED, waits for the AI's incoming
-- action, and sends ACCEPTED only after an unchanged accepted offer.
function B.BeginCaptiveExchange()
    local c=EnemyConfig();
    assert(not captiveExchange and not Game:GetProperty("JAC_BOUNDARY_CAPTIVE_DEAL_ATTEMPT"),
        "native captive exchange already attempted");
    local d=B.EnemyRetryDiagnosis();
    assert(d.mode=="UNAVAILABLE_CAPACITY" and d.outcome=="CAPTURED" and
        d.occupied==1 and d.capacity==1 and d.captive.captor==c.owner,
        "native captive exchange needs the original occupied spy slot");
    local captor=d.captive.captor;
    assert(not Players[c.attacker]:GetDiplomacy():IsAtWarWith(captor) and
        not DealManager.HasPendingDeal(c.attacker,captor),
        "native captive deal is at war or already pending");
    local deal=assert(DealManager.GetWorkingDeal(DealDirection.OUTGOING,c.attacker,captor),
        "native outgoing working deal unavailable");
    assert(#WorkingDealItemIDs(deal)==0,"existing unsent native deal must be preserved");
    local item=assert(deal:AddItemOfType(DealItemTypes.CAPTIVE,captor),
        "native captive deal item unavailable");
    local itemID=item:GetID();
    assert(type(itemID)=="number" and itemID>=0,"native captive deal item ID unavailable");
    local ok,err=pcall(function()
        item:SetValueType(d.captive.nameIndex);
        assert(item:IsValid(),"native captive deal item invalid");
        item:SetLocked(true);
        local observed=ExactCaptiveDealItem(deal,captor,c.attacker,d.captive.nameIndex);
        assert(observed and observed:IsLocked(),"native captive deal item changed before session");
        DiplomacyManager.RequestSession(c.attacker,captor,"MAKE_DEAL");
    end);
    if not ok then
        RemovePreparedCaptiveItem(item,captor,c.attacker);
        error(err);
    end
    captiveExchange={attacker=c.attacker,captor=captor,nameIndex=d.captive.nameIndex,
        captiveName=d.captive.name,captiveLevel=d.captive.level,
        itemID=itemID,spyID=d.spyID,eventID=d.eventID,eventTurn=d.eventTurn,
        sessionRequested=true,proposed=false,finalSent=false};
    Log("ENEMY_CAPTIVE_EXCHANGE_SESSION",captiveExchange);
    return "SESSION_REQUESTED";
end

local function CaptiveExchangeSession()
    local x=assert(captiveExchange,"native captive exchange session missing");
    local id=DiplomacyManager.FindOpenSessionID(x.attacker,x.captor);
    if id==nil then return nil; end
    local info=assert(DiplomacyManager.GetSessionInfo(id),
        "native captive diplomacy session info missing");
    assert(info.FromPlayer==x.attacker,"native captive deal was not initiated by the attacker");
    return id;
end

local function MapSpyNativeRouteReady(unit,c)
    local plot=Map.GetPlot(unit:GetX(),unit:GetY());
    local city=plot and Cities.GetPlotPurchaseCity(plot);
    if not city then return false; end
    if city:GetOwner()==c.attacker then
        local target=assert(CityManager.GetCity(c.owner,c.cityID),
            "native enemy Governor destination missing");
        local params={[UnitOperationTypes.PARAM_X]=target:GetX(),
            [UnitOperationTypes.PARAM_Y]=target:GetY()};
        return UnitManager.CanStartOperation(unit,UnitOperationTypes.SPY_TRAVEL_NEW_CITY,
            Map.GetPlot(target:GetX(),target:GetY()),params);
    end
    if city:GetOwner()==c.owner and city:GetID()==c.cityID then
        local operation=assert(GameInfo.UnitOperations["UNITOPERATION_SPY_NEUTRALIZE_GOVERNOR"],
            "native Governor neutralization operation missing");
        return UnitManager.CanStartOperation(unit,operation.Hash,plot,false,true);
    end
    return false;
end

function B.CaptiveExchangeState()
    local x=captiveExchange;
    if not x then
        return {stage=Game:GetProperty("JAC_BOUNDARY_CAPTIVE_DEAL_ATTEMPT") and
            "ATTEMPT_RECORDED" or "NOT_STARTED"};
    end
    if not x.proposed then
        return {stage=CaptiveExchangeSession() and "SESSION_OPEN" or "SESSION_PENDING"};
    end
    local marker=Game:GetProperty("JAC_BOUNDARY_CAPTIVE_DEAL_ATTEMPT");
    if not marker then return {stage="PROPOSAL_RECEIPT_PENDING"}; end
    assert(marker.spyID==x.spyID and marker.eventID==x.eventID and
        marker.captor==x.captor and marker.nameIndex==x.nameIndex,
        "native captive proposal receipt changed");
    if x.invalid then return {stage="DEAL_INVALID"}; end
    if x.response==DealProposalAction.REJECTED then
        return {stage=x.finalSent and "FINAL_REFUSED" or "AI_REFUSED"};
    end
    if x.response==DealProposalAction.ADJUSTED or
        x.response==DealProposalAction.PROPOSED then
        return {stage="AI_CHANGED_TERMS"};
    end
    if x.response~=nil and x.response~=DealProposalAction.PENDING and
        x.response~=DealProposalAction.ACCEPTED then
        return {stage="AI_RESPONSE_UNCLASSIFIED"};
    end
    if not x.finalSent then
        if x.response~=DealProposalAction.ACCEPTED then
            return {stage="PROPOSED_WAIT_AI"};
        end
        local incoming=DealManager.GetWorkingDeal(DealDirection.INCOMING,x.attacker,x.captor);
        local outgoing=DealManager.GetWorkingDeal(DealDirection.OUTGOING,x.attacker,x.captor);
        local incomingItem=incoming and ExactCaptiveDealItem(incoming,x.captor,x.attacker,x.nameIndex);
        local outgoingItem=outgoing and ExactCaptiveDealItem(outgoing,x.captor,x.attacker,x.nameIndex);
        if not (incomingItem and outgoingItem and
            DealManager.AreWorkingDealsEqual(x.attacker,x.captor)) then
            return {stage="AI_CHANGED_TERMS"};
        end
        if DealManager.HasPendingDeal(x.attacker,x.captor) then
            return {stage="AI_ACCEPTED_PENDING"};
        end
        return {stage="AI_ACCEPTED_EXACT"};
    end
    local roster=ReadSpyRoster(x.attacker);
    Log("ENEMY_CAPTIVE_EXCHANGE_RETURN_ROSTER",roster);
    if #roster.captured~=0 then return {stage="AWAIT_NATIVE_RETURN"}; end
    if #roster.offMap==1 then
        local record=roster.offMap[1];
        if record.returnTurn==-1 and record.x==-9999 and record.y==-9999 and
            record.name==x.captiveName and record.level==x.captiveLevel then
            return {stage="RETURN_UNOBSERVED"};
        end
    end
    if #roster.map==0 and #roster.activeOffMap==0 then
        return {stage="RETURN_UNOBSERVED"};
    end
    local d=B.EnemyRetryDiagnosis();
    assert(d.mode=="MAP" or d.mode=="RETURN",
        "accepted native captive deal has no living original operative");
    local ready=false;
    if d.mode=="MAP" then
        local unit=assert(Players[x.attacker]:GetUnits():FindID(d.currentID),
            "returned native map spy missing");
        ready=MapSpyNativeRouteReady(unit,EnemyConfig());
    end
    return {stage=d.mode=="RETURN" and "RETURNED_OFF_MAP" or
            ready and "RETURNED_MAP_READY" or "RETURNED_MAP_WAIT_NATIVE",
        spyID=d.spyID,currentID=d.currentID,capacity=d.capacity,
        occupied=d.occupied,operationReady=ready};
end

function B.ProposeCaptiveExchange()
    local x=assert(captiveExchange,"native captive exchange session missing");
    assert(not x.proposed and CaptiveExchangeSession(),
        "native captive exchange session is not open");
    local marker=assert(Game:GetProperty("JAC_BOUNDARY_CAPTIVE_DEAL_ATTEMPT"),
        "native captive exchange attempt was not recorded");
    assert(marker.spyID==x.spyID and marker.captor==x.captor and
        marker.nameIndex==x.nameIndex,"native captive exchange receipt differs");
    local deal=assert(DealManager.GetWorkingDeal(DealDirection.OUTGOING,x.attacker,x.captor),
        "native captive deal missing before proposal");
    local item=assert(ExactCaptiveDealItem(deal,x.captor,x.attacker,x.nameIndex),
        "native captive deal terms changed before proposal");
    assert(item:IsLocked() and item:GetID()==x.itemID,
        "native captive deal lock or item identity changed");
    assert(not DealManager.HasPendingDeal(x.attacker,x.captor) and
        not Players[x.attacker]:GetDiplomacy():IsAtWarWith(x.captor),
        "native captive proposal became unavailable");
    x.proposed=true;
    if not deal:Validate() then
        x.invalid=true;
        RemovePreparedCaptiveItem(item,x.captor,x.attacker);
        return "DEAL_INVALID";
    end
    DealManager.SendWorkingDeal(DealProposalAction.PROPOSED,x.attacker,x.captor);
    Log("ENEMY_CAPTIVE_EXCHANGE_PROPOSED",{attacker=x.attacker,captor=x.captor,
        spyID=x.spyID,eventID=x.eventID,nameIndex=x.nameIndex});
    return "PROPOSED_WAIT_AI";
end

function B.AcceptCaptiveExchange()
    local x=assert(captiveExchange,"native captive exchange session missing");
    assert(not x.finalSent and B.CaptiveExchangeState().stage=="AI_ACCEPTED_EXACT",
        "native AI did not accept the exact captive deal");
    assert(CaptiveExchangeSession() and
        not DealManager.HasPendingDeal(x.attacker,x.captor) and
        not Players[x.attacker]:GetDiplomacy():IsAtWarWith(x.captor),
        "native captive final acceptance became unavailable");
    x.finalSent=true;
    DealManager.SendWorkingDeal(DealProposalAction.ACCEPTED,x.attacker,x.captor);
    Log("ENEMY_CAPTIVE_EXCHANGE_FINAL_SENT",{attacker=x.attacker,captor=x.captor,
        spyID=x.spyID,eventID=x.eventID,nameIndex=x.nameIndex});
    return "FINAL_SENT";
end

function B.CloseCaptiveExchangeSession()
    local x=assert(captiveExchange,"native captive exchange session missing");
    local stage=B.CaptiveExchangeState().stage;
    assert(stage=="RETURNED_MAP_READY" or stage=="RETURNED_MAP_WAIT_NATIVE" or
        stage=="RETURNED_OFF_MAP" or stage=="AI_REFUSED" or
        stage=="AI_CHANGED_TERMS" or stage=="AI_RESPONSE_UNCLASSIFIED" or
        stage=="DEAL_INVALID" or stage=="FINAL_REFUSED",
        "native captive deal is not in a classified terminal state");
    local id=DiplomacyManager.FindOpenSessionID(x.attacker,x.captor);
    if id then DiplomacyManager.CloseSession(id); end
    Log("ENEMY_CAPTIVE_EXCHANGE_SESSION_CLOSED",{stage=stage,sessionID=id});
    return stage;
end

-- The cascaded official database has one already used spy-granting Civic and
-- the Government Plaza Intelligence Agency building. Read the actual city
-- prerequisites before offering the building path; a captive still uses one
-- capacity slot throughout this path.
local function GovernmentDistrictPlacement(owner,district)
    local options={};
    for _,city in Players[owner]:GetCities():Members() do
        local districts=city:GetDistricts();
        local queue=city:GetBuildQueue();
        if not districts:HasDistrict(district.Index,true) then
            local head=queue:GetAt(0);
            if head and head.DistrictType==district.Index then
                return {stage="DISTRICT_QUEUED",cityID=city:GetID()};
            end
            if districts:HasDistrict(district.Index) then
                return {stage="DISTRICT_PLACED_PENDING",cityID=city:GetID()};
            end
            if queue:CanProduce(district.Hash,false,true) then
                local arguments={[CityOperationTypes.PARAM_DISTRICT_TYPE]=district.Hash};
                local targets=CityManager.GetOperationTargets(city,CityOperationTypes.BUILD,arguments);
                local plots=targets and targets[CityOperationResults.PLOTS];
                if type(plots)=="table" then for _,plotID in ipairs(plots) do
                    local plot=Map.GetPlotByIndex(plotID);
                    local purchase=plot and Cities.GetPlotPurchaseCity(plot);
                    if purchase and purchase:GetOwner()==owner and
                        purchase:GetID()==city:GetID() then
                        arguments[CityOperationTypes.PARAM_X]=plot:GetX();
                        arguments[CityOperationTypes.PARAM_Y]=plot:GetY();
                        arguments[CityOperationTypes.PARAM_INSERT_MODE]=CityOperationTypes.VALUE_EXCLUSIVE;
                        local valid=CityManager.CanStartOperation(city,CityOperationTypes.BUILD,
                            arguments,true);
                        if valid then options[#options+1]={stage="BUILD_DISTRICT",
                            cityID=city:GetID(),plot=plotID,x=plot:GetX(),y=plot:GetY()}; end
                    end
                end end
            end
        end
    end
    table.sort(options,function(a,b)
        if a.cityID~=b.cityID then return a.cityID<b.cityID; end
        return a.plot<b.plot;
    end);
    return options[1];
end

function B.CaptiveCapacityPlan()
    local c=EnemyConfig();
    local d=B.EnemyRetryDiagnosis();
    assert(d.outcome=="CAPTURED" and d.captive and d.occupied==1,
        "capacity plan requires the original native captive");
    local result={capacity=d.capacity,occupied=d.occupied,stage="UNAVAILABLE",
        cities={}};
    if d.capacity>1 then
        result.stage="CAPACITY_READY";
        Log("ENEMY_CAPTIVE_CAPACITY_PLAN",{plan=result,roster=d.roster});
        return result;
    end
    local building=assert(GameInfo.Buildings["BUILDING_GOV_SPIES"],
        "official spy capacity building missing");
    local district=assert(GameInfo.Districts["DISTRICT_GOVERNMENT"],
        "official Government Plaza missing");
    assert(building.PrereqDistrict==district.DistrictType,
        "spy capacity building district prerequisite changed");
    local culture=Players[c.attacker]:GetCulture();
    local civic=assert(GameInfo.Civics[district.PrereqCivic],
        "Government Plaza civic prerequisite missing");
    result.districtCivic=district.PrereqCivic;
    result.districtCivicOwned=culture:HasCivic(civic.Index);
    local tier1={"BUILDING_GOV_TALL","BUILDING_GOV_CONQUEST","BUILDING_GOV_WIDE"};
    local plazaSeen=false;
    for _,city in Players[c.attacker]:GetCities():Members() do
        local districts=assert(city:GetDistricts(),"native city districts unavailable");
        local buildings=assert(city:GetBuildings(),"native city buildings unavailable");
        local observed={cityID=city:GetID(),district=districts:HasDistrict(district.Index,true),
            placed=districts:HasDistrict(district.Index),
            tier1={},spyBuilding=buildings:HasBuilding(building.Index)};
        for _,kind in ipairs(tier1) do
            local row=assert(GameInfo.Buildings[kind],"official Government Plaza tier-one building missing");
            observed.tier1[kind]=buildings:HasBuilding(row.Index);
        end
        result.cities[#result.cities+1]=observed;
        if observed.district then
            assert(not plazaSeen,"native Government Plaza belongs to multiple cities");
            plazaSeen=true;
        end
        if observed.spyBuilding then
            result.stage="EFFECT_PENDING"; result.cityID=city:GetID();
        elseif observed.district and result.stage~="EFFECT_PENDING" then
            local choice,stage;
            for _,kind in ipairs(tier1) do if observed.tier1[kind] then
                choice=building; stage="BUILD_SPIES"; break;
            end end
            if not choice then
                local head=city:GetBuildQueue():GetAt(0);
                for _,kind in ipairs(tier1) do
                    local row=GameInfo.Buildings[kind];
                    if head and head.BuildingType==row.Index then
                        choice=row; stage="BUILD_TIER1"; break;
                    end
                end
            end
            if not choice then
                for _,kind in ipairs(tier1) do
                    local row=GameInfo.Buildings[kind];
                    local can=city:GetBuildQueue():CanProduce(row.Hash,false,true);
                    if can then choice=row; stage="BUILD_TIER1"; break; end
                end
            end
            if choice then
                local head=city:GetBuildQueue():GetAt(0);
                local queued=head and head.BuildingType==choice.Index;
                local progress=Game:GetProperty("JAC_BOUNDARY_CAPACITY_PROGRESS");
                local progressed=progress and progress.steps and progress.steps[choice.BuildingType];
                local can=city:GetBuildQueue():CanProduce(choice.Hash,false,true);
                if queued and progressed then stage="PROGRESS_PENDING";
                elseif queued then stage="BUILD_QUEUED";
                elseif not can then stage="BUILDING_UNAVAILABLE"; end
                result.stage=stage; result.cityID=city:GetID();
                result.buildingType=choice.BuildingType;
            elseif result.stage=="UNAVAILABLE" then result.stage="TIER1_UNAVAILABLE"; end
        end
    end
    if result.stage=="UNAVAILABLE" then
        if result.districtCivicOwned then
            local placement=GovernmentDistrictPlacement(c.attacker,district);
            if placement then for key,value in pairs(placement) do result[key]=value; end
            else result.stage="DISTRICT_UNAVAILABLE"; end
        else result.stage="NEEDS_DOMAIN_LAW"; end
    end
    Log("ENEMY_CAPTIVE_CAPACITY_PLAN",{plan=result,roster=d.roster});
    return result;
end

-- A progress receipt is not a completed building. Read the exact latest
-- target building and the native capacity after a real turn, with the captive
-- still present, before a scenario publishes its checkpoint.
function B.CaptiveCapacityOutcome()
    local c=EnemyConfig();
    local progress=assert(Game:GetProperty("JAC_BOUNDARY_CAPACITY_PROGRESS"),
        "native capacity building progress receipt missing");
    local latest=assert(progress.latest,"latest capacity building receipt missing");
    local step=assert(progress.steps and progress.steps[latest.buildingType],
        "latest capacity building step missing");
    assert(step.cityID==latest.cityID and step.buildingIndex==latest.buildingIndex and
        step.token==latest.token and step.turn==latest.turn,
        "native capacity building receipt changed");
    local row=assert(GameInfo.Buildings[latest.buildingType],
        "native capacity target building missing");
    assert(row.Index==latest.buildingIndex,
        "native capacity target building type changed");
    local city=assert(CityManager.GetCity(c.attacker,latest.cityID),
        "native capacity production city missing");
    assert(city:GetOwner()==c.attacker,"native capacity production city owner changed");
    local d=B.EnemyRetryDiagnosis();
    assert(d.outcome=="CAPTURED" and d.occupied==1 and #d.roster.captured==1,
        "original native captive changed during capacity construction");
    local built=city:GetBuildings():HasBuilding(row.Index);
    local stage="PROGRESS_PENDING";
    if row.BuildingType=="BUILDING_GOV_SPIES" then
        if built then stage=d.capacity==2 and "CAPACITY_READY" or "EFFECT_PENDING"; end
    else
        assert(row.BuildingType=="BUILDING_GOV_TALL" or
            row.BuildingType=="BUILDING_GOV_CONQUEST" or
            row.BuildingType=="BUILDING_GOV_WIDE",
            "latest progress is not a Government Plaza prerequisite");
        if built then stage=d.capacity==1 and "TIER1_BUILT" or "CAPACITY_CHANGED"; end
    end
    local result={stage=stage,buildingType=row.BuildingType,cityID=city:GetID(),
        buildingIndex=row.Index,targetBuilt=built,capacity=d.capacity,
        occupied=d.occupied,captured=#d.roster.captured,
        turn=Game.GetCurrentGameTurn(),progressTurn=latest.turn};
    Log("ENEMY_CAPTIVE_CAPACITY_OUTCOME",{result=result,roster=d.roster});
    return result;
end

function B.CaptiveCapacityDistrictOutcome()
    local c=EnemyConfig();
    local receipt=assert(Game:GetProperty("JAC_BOUNDARY_CAPACITY_DISTRICT_PROGRESS"),
        "native Government Plaza progress receipt missing");
    assert(receipt.owner==c.attacker and
        receipt.districtIndex==GameInfo.Districts["DISTRICT_GOVERNMENT"].Index,
        "native Government Plaza progress identity changed");
    local city=assert(CityManager.GetCity(c.attacker,receipt.cityID),
        "native Government Plaza city missing");
    local plot=assert(Map.GetPlot(receipt.x,receipt.y),
        "native Government Plaza plot missing");
    local purchase=Cities.GetPlotPurchaseCity(plot);
    assert(city:GetOwner()==c.attacker and plot:GetIndex()==receipt.plot and
        purchase and purchase:GetOwner()==c.attacker and
        purchase:GetID()==city:GetID(),
        "native Government Plaza owner or plot changed");
    local d=B.EnemyRetryDiagnosis();
    assert(d.outcome=="CAPTURED" and d.occupied==1 and d.capacity==1 and
        #d.roster.captured==1,"native captive capacity changed before district completion");
    local districts=city:GetDistricts();
    local built=districts:HasDistrict(receipt.districtIndex,true);
    local atPlot=false;
    for _,district in districts:Members() do
        if district:GetType()==receipt.districtIndex and
            district:GetX()==receipt.x and district:GetY()==receipt.y then
            assert(not atPlot,"native Government Plaza district duplicated");
            atPlot=true;
        end
    end
    local stage=built and atPlot and "DISTRICT_BUILT" or
        built and "DISTRICT_PLOT_MISMATCH" or "PROGRESS_PENDING";
    local result={stage=stage,cityID=city:GetID(),plot=receipt.plot,
        x=receipt.x,y=receipt.y,targetBuilt=built and atPlot,
        capacity=d.capacity,occupied=d.occupied,captured=#d.roster.captured,
        turn=Game.GetCurrentGameTurn(),progressTurn=receipt.turn};
    Log("ENEMY_CAPTIVE_DISTRICT_OUTCOME",{result=result,roster=d.roster});
    return result;
end

function B.RetryLivingSpy()
    local c=EnemyConfig();
    local retry=assert(CheckArchivedMission(c),"archived retry missing");
    assert(retry.mode=="MAP" or retry.mode=="RETURN",
        "retry is not following a living native operative");
    local state=B.ReadEnemy();
    R.Check(state,"SETTLED");
    local roster=ReadSpyRoster(c.attacker);
    Log("ENEMY_SPY_RETRY_LIVING",{retry=retry,roster=roster});
    assert(#roster.captured==0,"native retry spy became captive; return identity unavailable");
    assert(roster.physicalCount<=1,"living retry operative roster is ambiguous");
    if #roster.activeOffMap==1 and #roster.map==0 then
        local record=roster.activeOffMap[1];
        assert(retry.mode=="RETURN" and
            record.name==retry.returnName and record.level==retry.returnLevel and
            record.returnTurn==retry.returnTurn and
            record.x==retry.returnX and record.y==retry.returnY,
            "native return record changed or belongs to another spy");
        return "OFF_MAP_RETURN";
    end
    if #roster.map==0 and #roster.activeOffMap==0 then return "ARRIVAL_UNOBSERVED"; end
    assert(#roster.map==1 and #roster.activeOffMap==0 and #roster.captured==0,
        "living retry operative roster is ambiguous");
    local unit=roster.map[1];
    if retry.mode=="RETURN" then
        local expected=retry.returnName=="" and GameInfo.Units["UNIT_SPY"].Name or
            retry.returnName;
        assert(unit.name==expected and unit.level==retry.returnLevel and
            unit.x==retry.returnX and unit.y==retry.returnY,
            "returned native operative differs from the off-map receipt");
        if unit.id~=retry.spyID then
            return "ADOPTION_REQUIRED";
        end
    else
        assert(unit.id==retry.spyID,"retry map spy identity changed");
    end
    local city=Cities.GetPlotPurchaseCity(Map.GetPlot(unit.x,unit.y));
    if city and city:GetOwner()==c.attacker then
        local native=assert(Players[c.attacker]:GetUnits():FindID(unit.id),
            "living retry map spy vanished before native travel query");
        return MapSpyNativeRouteReady(native,c) and
            "HOME_READY" or "WAIT_NATIVE_READY";
    end
    if city and city:GetOwner()==c.owner and city:GetID()==c.cityID then
        local probe=B.Probe(c.owner,c.cityID);
        return probe.canStart and "MISSION_READY" or "WAIT_NATIVE_READY";
    end
    return "WAIT_NATIVE_RETURN";
end

function B.TryAdoptReturnedSpy()
    assert(B.RetryLivingSpy()=="ADOPTION_REQUIRED",
        "a newly arrived native spy is required before identity adoption");
    local roster=ReadSpyRoster(Game.GetLocalPlayer());
    B.Queue("RETRY_ADOPT",{id=roster.map[1].id});
    return "ADOPT_QUEUED";
end

function B.RetryProducedSpyVisible()
    local retry=assert(Game:GetProperty(RETRY),"archived retry missing");
    assert(retry.current==2 and retry.mode=="PRODUCE" and
        Game:GetProperty("JAC_BOUNDARY_SPY_PROGRESS"),
        "retry production has not received one native progress request");
    local roster=ReadSpyRoster(Game.GetLocalPlayer());
    assert(#roster.activeOffMap==0 and #roster.map<=1 and
        roster.physicalCount==#roster.captured+#roster.map,
        "native production roster is ambiguous");
    if retry.spyID then
        assert(#roster.map==1 and roster.map[1].id==retry.spyID,
            "bound native retry spy disappeared from the map roster");
        return true;
    end
    if #roster.map==0 then return false; end
    assert(roster.map[1].id~=retry.archive.spyID,
        "old failed spy was mistaken for new native production");
    return true;
end

function B.TryBindRetrySpy()
    if not B.RetryProducedSpyVisible() then return "PRODUCTION_PENDING"; end
    local retry=Game:GetProperty(RETRY);
    if retry.spyID then return "BOUND"; end
    B.Queue("BIND_SPY"); return "BIND_QUEUED";
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

local pending, verification, buildPending, travelReference, capacityBuildReady,
    capacityDistrictReady;
local spyQueued = {};
local capacityQueued = {};
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
        if roster and #roster.activeOffMap==1 then stage="TRAVEL";
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
    local roster=ReadSpyRoster(Game.GetLocalPlayer());
    local existing=roster.physicalCount;
    Log("SPY_PRODUCTION_PREREQUISITES", {civic=unit.PrereqCivic, capacity=capacity, existing=existing});
    local retry=Game:GetProperty(RETRY);
    if retry then
        assert(retry.current==2 and retry.mode=="PRODUCE" and retry.archive and
            #roster.map==0 and #roster.activeOffMap==0 and existing==#roster.captured,
            "retry production requires a proven free native spy slot");
    else
        assert(existing==0, "spy production fixture needs no pre-existing native spies");
    end
    assert(type(capacity)=="number" and capacity>existing, "native spy capacity unavailable");
    return prepared;
end

local function SpyBuildReady(owner, cityID)
    local city=assert(CityManager.GetCity(owner,cityID), "spy production city missing");
    local head=city:GetBuildQueue():GetAt(0);
    local unit=head and head.UnitType and GameInfo.Units[head.UnitType];
    return unit and unit.UnitType=="UNIT_SPY" or false;
end
local function CapacityBuildReady(owner,cityID,buildingType)
    local city=assert(CityManager.GetCity(owner,cityID),"capacity building city missing");
    local head=city:GetBuildQueue():GetAt(0);
    local row=head and head.BuildingType and GameInfo.Buildings[head.BuildingType];
    return row and row.BuildingType==buildingType or false;
end
local function CapacityDistrictBuildReady(owner,cityID,districtType)
    local city=assert(CityManager.GetCity(owner,cityID),"capacity district city missing");
    local head=city:GetBuildQueue():GetAt(0);
    local row=head and head.DistrictType and GameInfo.Districts[head.DistrictType];
    return row and row.DistrictType==districtType or false;
end
-- Gameplay GetAt was nil in live 50. Gameplay can call this UI-owned read
-- bridge immediately before its AddProgress; only scalar queue identity crosses.
function B.ReadCapacityQueue(owner,cityID)
    assert(owner==Game.GetLocalPlayer(),"capacity queue reader owner differs");
    local city=assert(CityManager.GetCity(owner,cityID),"capacity queue city missing");
    local head=city:GetBuildQueue():GetAt(0);
    local row=head and head.BuildingType and GameInfo.Buildings[head.BuildingType];
    local district=head and head.DistrictType and GameInfo.Districts[head.DistrictType];
    local result={owner=city:GetOwner(),cityID=city:GetID(),
        buildingType=row and row.BuildingType or "",
        districtType=district and district.DistrictType or "",
        turn=Game.GetCurrentGameTurn(),frame=frameSerial,publishEpoch=publishEpoch};
    Log("CAPACITY_QUEUE_READ",result);
    return result;
end
ExposedMembers.JAC_PersonaBoundaryCapacityQueueRead=B.ReadCapacityQueue;
function B.Queue(action, params)
    assert(not pending and not verification and not buildPending and not enemyPending and not enemyVerification,
        "a boundary frame request is already pending");
    assert(Players[Game.GetLocalPlayer()]:IsTurnActive(), "boundary request needs the local active turn");
    assert(action == "PREPARE" or action == "PREPARE_ENEMY" or action == "EVOLVE_ENEMY" or
        action == "CONFIRM_ENEMY" or action == "BEGIN_SPY_RETRY" or
        action == "MARK_CAPTIVE_EXCHANGE" or
        action == "PREPARE_DISTRICT_CIVIC" or
        action == "CAPTURE_ENEMY" or action == "SAVE_TRAVEL" or
        action == "EVOLVE" or action == "ASSIGN" or action == "BUY" or
        action == "LOSE_CITY" or action == "SPAWN_SPY" or action == "BUILD_SPY" or
        action == "BUILD_CAPACITY" or action == "PROGRESS_CAPACITY" or
        action == "BUILD_CAPACITY_DISTRICT" or action == "PROGRESS_CAPACITY_DISTRICT" or
        action == "PROGRESS_SPY" or action == "BIND_SPY" or action == "ADOPT_SPY" or action == "SPY_TRAVEL" or
        action == "SPY_MISSION" or action == "RETRY_ADOPT", "unknown boundary action");
    local retry=Game:GetProperty(RETRY);
    if retry and action=="SPY_MISSION" then
        local spy=assert(Game:GetProperty("JAC_BOUNDARY_SPY"),"retry spy missing");
        assert(retry.current==2 and retry.spyID==spy.id and not retry.missionRequested and
            not Game:GetProperty("JAC_BOUNDARY_ENEMY_MISSION"),
            "retry native mission has already been requested or completed");
    end
    if retry and action=="SPY_TRAVEL" then
        assert(retry.spyID and not Game:GetProperty("JAC_BOUNDARY_SPY_TRAVEL"),
            "retry native travel has already been accepted");
    end
    if action=="MARK_CAPTIVE_EXCHANGE" then
        assert(captiveExchange and not captiveExchange.proposed and
            not Game:GetProperty("JAC_BOUNDARY_CAPTIVE_DEAL_ATTEMPT"),
            "native captive exchange attempt already recorded");
    end
    if action == "SPAWN_SPY" or action == "BUILD_SPY" or action == "PROGRESS_SPY" or
        action == "BIND_SPY" or action == "ADOPT_SPY" or action == "RETRY_ADOPT" or
        action == "SPY_TRAVEL" or action == "SPY_MISSION" then
        assert(not spyQueued[action], "native spy request cannot be replayed in this session");
        spyQueued[action] = true;
    end
    if action=="BUILD_CAPACITY" or action=="PROGRESS_CAPACITY" then
        local plan=B.CaptiveCapacityPlan();
        assert(plan.buildingType,"native capacity building prerequisite missing");
        if action=="PROGRESS_CAPACITY" then
            assert(capacityBuildReady and capacityBuildReady.cityID==plan.cityID and
                capacityBuildReady.buildingType==plan.buildingType and
                capacityBuildReady.turn==Game.GetCurrentGameTurn(),
                "native capacity BUILD acceptance is missing in this UI turn");
        end
        local key=action.."/"..plan.buildingType;
        assert(not capacityQueued[key],"native capacity building request cannot be replayed");
        capacityQueued[key]=true;
    end
    if action=="BUILD_CAPACITY_DISTRICT" or action=="PROGRESS_CAPACITY_DISTRICT" then
        local plan=B.CaptiveCapacityPlan();
        if action=="PROGRESS_CAPACITY_DISTRICT" then
            assert(plan.stage=="DISTRICT_QUEUED" and capacityDistrictReady and
                capacityDistrictReady.cityID==plan.cityID and
                capacityDistrictReady.turn==Game.GetCurrentGameTurn(),
                "native district BUILD acceptance is missing in this UI turn");
        else
            assert(plan.stage=="BUILD_DISTRICT" and plan.plot,
                "native Government Plaza has no proven owned placement");
        end
        assert(not capacityQueued[action],"native Government Plaza request cannot be replayed");
        capacityQueued[action]=true;
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
        CAPTURE_ENEMY="JAC_BoundaryCaptureEnemyRecovery",
        BEGIN_SPY_RETRY="JAC_BoundaryBeginEnemySpyRetry",
        MARK_CAPTIVE_EXCHANGE="JAC_BoundaryMarkCaptiveExchange",
        PREPARE_DISTRICT_CIVIC="JAC_BoundaryPrepareCapacityCivic",
        PROGRESS_CAPACITY_DISTRICT="JAC_BoundaryProgressCapacityDistrict",
        PROGRESS_CAPACITY="JAC_BoundaryProgressCapacityBuilding"};
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
    elseif action == "MARK_CAPTIVE_EXCHANGE" then
        local x=assert(captiveExchange,"native captive exchange session missing");
        assert(not x.proposed and CaptiveExchangeSession(),
            "native captive exchange session is not open");
        return EnemySyncRequest(action,{Captor=x.captor,NameIndex=x.nameIndex,
            SpyID=x.spyID,EventID=x.eventID,EventTurn=x.eventTurn});
    elseif action == "PREPARE_DISTRICT_CIVIC" then
        local plan=B.CaptiveCapacityPlan();
        assert(plan.stage=="NEEDS_DOMAIN_LAW" and not plan.districtCivicOwned,
            "native Government Plaza civic is already complete or unavailable");
        local civic=assert(GameInfo.Civics["CIVIC_JAC_SENGOKU_DOMAIN_LAW"],
            "native Government Plaza civic missing");
        local titles=Players[owner]:GetGovernors();
        return EnemySyncRequest(action,{CivicIndex=civic.Index,
            Total=titles:GetGovernorPoints(),Spent=titles:GetGovernorPointsSpent()});
    elseif action == "PROGRESS_CAPACITY_DISTRICT" then
        local plan=B.CaptiveCapacityPlan();
        local ready=assert(capacityDistrictReady,"native Government Plaza BUILD not accepted");
        assert(plan.stage=="DISTRICT_QUEUED" and plan.cityID==ready.cityID and
            ready.turn==Game.GetCurrentGameTurn(),
            "native Government Plaza queue changed before progress");
        local district=assert(GameInfo.Districts["DISTRICT_GOVERNMENT"],
            "native Government Plaza definition missing");
        return EnemySyncRequest(action,{CityID=ready.cityID,
            DistrictIndex=district.Index,DistrictType=district.DistrictType,
            PlotIndex=ready.plot,X=ready.x,Y=ready.y,
            Capacity=plan.capacity,Occupied=plan.occupied});
    elseif action == "PROGRESS_CAPACITY" then
        local plan=B.CaptiveCapacityPlan();
        assert(plan.stage=="BUILD_QUEUED" and plan.cityID and plan.buildingType,
            "official capacity building is not queued for progress");
        assert(capacityBuildReady and capacityBuildReady.cityID==plan.cityID and
            capacityBuildReady.buildingType==plan.buildingType and
            capacityBuildReady.turn==Game.GetCurrentGameTurn(),
            "native capacity BUILD acceptance changed before progress");
        local row=assert(GameInfo.Buildings[plan.buildingType],"native capacity building missing");
        return EnemySyncRequest(action,{CityID=plan.cityID,BuildingType=plan.buildingType,
            BuildingIndex=row.Index,Capacity=plan.capacity,Occupied=plan.occupied});
    elseif action == "BEGIN_SPY_RETRY" then
        local d=B.EnemyRetryDiagnosis();
        assert(d.mode=="PRODUCE" or d.mode=="RETURN" or d.mode=="MAP",
            "native retry route unavailable: "..d.mode);
        return EnemySyncRequest(action,{Mode=d.mode,Outcome=d.outcome,
            SpyID=d.spyID,CurrentID=d.currentID,EventID=d.eventID,EventTurn=d.eventTurn,
            Operation=d.mission.operation,Plot=d.mission.plot,
            Initial=d.mission.initial,Escape=d.mission.escape,
            CompletionTurn=d.mission.turn,Spent=d.spent,Total=d.total,
            GovernorType=d.governorType,Capacity=d.capacity,Occupied=d.occupied,
            CaptiveCaptor=d.captive and d.captive.captor,
            CaptiveName=d.captive and d.captive.name,
            CaptiveNameIndex=d.captive and d.captive.nameIndex,
            CaptiveLevel=d.captive and d.captive.level,
            ReturnName=d.returnName,ReturnLevel=d.returnLevel,
            ReturnTurn=d.returnTurn,ReturnX=d.returnX,ReturnY=d.returnY});
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
    elseif action == "BUILD_CAPACITY" then
        local plan=B.CaptiveCapacityPlan();
        assert((plan.stage=="BUILD_TIER1" or plan.stage=="BUILD_SPIES") and
            plan.cityID and plan.buildingType,
            "official capacity building prerequisites are not ready");
        local city=assert(CityManager.GetCity(owner,plan.cityID),"capacity city missing");
        local row=assert(GameInfo.Buildings[plan.buildingType],"capacity building missing");
        assert(not row.RequiresPlacement,"capacity building requires native placement UI");
        UI.DeselectAllCities(); UI.SelectCity(city);
        local selected=UI.GetHeadSelectedCity();
        assert(selected and selected:GetOwner()==owner and selected:GetID()==city:GetID(),
            "official UI did not select the capacity building city");
        local canProduce,reasons=city:GetBuildQueue():CanProduce(row.Hash,false,true);
        Log("CAPACITY_CAN_PRODUCE",{ok=canProduce,reasons=reasons or {},
            cityID=city:GetID(),buildingType=row.BuildingType});
        assert(canProduce,"native city refuses capacity building production");
        local arguments={[CityOperationTypes.PARAM_BUILDING_TYPE]=row.Hash,
            [CityOperationTypes.PARAM_INSERT_MODE]=CityOperationTypes.VALUE_REPLACE_AT,
            [CityOperationTypes.PARAM_QUEUE_DESTINATION_LOCATION]=0};
        local canStart,result=CityManager.CanStartOperation(city,CityOperationTypes.BUILD,arguments,true);
        Log("CAPACITY_BUILD_QUERY",{ok=canStart,result=result or {},
            cityID=city:GetID(),buildingType=row.BuildingType});
        assert(canStart,"native capacity building BUILD operation refused");
        buildPending={owner=owner,cityID=city:GetID(),buildingType=row.BuildingType,
            action="BUILD_CAPACITY",seen=false};
        CityManager.RequestOperation(city,CityOperationTypes.BUILD,arguments);
    elseif action == "BUILD_CAPACITY_DISTRICT" then
        local plan=B.CaptiveCapacityPlan();
        assert(plan.stage=="BUILD_DISTRICT" and plan.cityID and plan.plot and
            plan.x and plan.y,"native Government Plaza placement changed");
        local city=assert(CityManager.GetCity(owner,plan.cityID),
            "owned Government Plaza city missing");
        local district=assert(GameInfo.Districts["DISTRICT_GOVERNMENT"],
            "native Government Plaza definition missing");
        UI.DeselectAllCities(); UI.SelectCity(city);
        local selected=UI.GetHeadSelectedCity();
        assert(selected and selected:GetOwner()==owner and selected:GetID()==city:GetID(),
            "official UI did not select the Government Plaza city");
        local targets=CityManager.GetOperationTargets(city,CityOperationTypes.BUILD,
            {[CityOperationTypes.PARAM_DISTRICT_TYPE]=district.Hash});
        local plots=targets and targets[CityOperationResults.PLOTS];
        local found=false;
        if type(plots)=="table" then for _,plot in ipairs(plots) do
            if plot==plan.plot then found=true; break; end
        end end
        assert(found,"Government Plaza plot is no longer a native BUILD target");
        local arguments={[CityOperationTypes.PARAM_X]=plan.x,
            [CityOperationTypes.PARAM_Y]=plan.y,
            [CityOperationTypes.PARAM_DISTRICT_TYPE]=district.Hash,
            [CityOperationTypes.PARAM_INSERT_MODE]=CityOperationTypes.VALUE_EXCLUSIVE};
        local canStart,result=CityManager.CanStartOperation(city,CityOperationTypes.BUILD,
            arguments,true);
        Log("CAPACITY_DISTRICT_BUILD_QUERY",{ok=canStart,result=result or {},
            cityID=city:GetID(),plot=plan.plot,x=plan.x,y=plan.y});
        assert(canStart,"native Government Plaza BUILD operation refused");
        buildPending={owner=owner,cityID=city:GetID(),districtType=district.DistrictType,
            plot=plan.plot,x=plan.x,y=plan.y,
            action="BUILD_CAPACITY_DISTRICT",seen=false};
        CityManager.RequestOperation(city,CityOperationTypes.BUILD,arguments);
    elseif action == "ADOPT_SPY" then
        local prior=assert(travelReference or Game:GetProperty("JAC_BOUNDARY_SPY_TRAVEL"),
            "accepted spy travel reference missing");
        local roster=ReadSpyRoster(owner);
        assert(#roster.activeOffMap==0 and #roster.map==1,
            "arrived native spy roster is ambiguous");
        local arrived=roster.map[1];
        assert(arrived.name==prior.name and arrived.level==prior.level and
            InTargetCity(arrived.x,arrived.y,prior.targetOwner,prior.targetID),
            "arrived native spy does not match the accepted travel");
        return {action=action,stage="ADOPT_PENDING",before={owner=owner,id=prior.oldID},
            newID=arrived.id,targetOwner=prior.targetOwner,targetID=prior.targetID};
    elseif action == "RETRY_ADOPT" then
        local retry=assert(CheckArchivedMission(c),"archived retry missing");
        assert(retry.mode=="RETURN" and retry.spyID and type(params.id)=="number",
            "returned native spy cannot be adopted");
        local arrived=assert(Players[owner]:GetUnits():FindID(params.id),
            "returned native spy unit missing");
        local city=assert(Cities.GetPlotPurchaseCity(Map.GetPlot(arrived:GetX(),arrived:GetY())),
            "returned native spy has no city");
        assert(arrived:GetType()==GameInfo.Units["UNIT_SPY"].Index and
            arrived:GetID()~=retry.spyID and arrived:GetX()==retry.returnX and
            arrived:GetY()==retry.returnY,
            "returned native spy differs from observed off-map arrival");
        return {action=action,stage="ADOPT_PENDING",before={owner=owner,id=retry.spyID},
            newID=arrived:GetID(),targetOwner=city:GetOwner(),targetID=city:GetID()};
    else
        local spy=LocalSpy(); local city=assert(CityManager.GetCity(params.owner,params.city), "spy target missing");
        assert(city:GetOwner() ~= owner, "spy target must be foreign");
        local plot=Map.GetPlot(city:GetX(),city:GetY());
        if action == "SPY_TRAVEL" then
            SelectSpy(spy, InterfaceModeTypes.SPY_TRAVEL_TO_CITY);
            local roster=ReadSpyRoster(owner);
            assert(#roster.map==1 and roster.map[1].id==spy:GetID() and #roster.activeOffMap==0,
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
        if #afterRoster.activeOffMap==1 and #afterRoster.map<=1 then
            local travelling=afterRoster.activeOffMap[1];
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
        elseif #afterRoster.activeOffMap==0 and #afterRoster.map==1 then
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
            elseif request.action=="BEGIN_SPY_RETRY" then
                local retry=Game:GetProperty(RETRY);
                accepted=retry and retry.current==2 and retry.mode==request.payload.Mode and
                    retry.archive and retry.archive.spyID==request.payload.SpyID and
                    retry.archive.eventID==request.payload.EventID and
                    Game:GetProperty("JAC_BOUNDARY_ENEMY_MISSION")==nil;
            elseif request.action=="PROGRESS_CAPACITY" then
                local progress=Game:GetProperty("JAC_BOUNDARY_CAPACITY_PROGRESS");
                local step=progress and progress.steps and
                    progress.steps[request.payload.BuildingType];
                accepted=step and step.token==request.token and
                    step.cityID==request.payload.CityID and
                    step.buildingIndex==request.payload.BuildingIndex;
            elseif request.action=="MARK_CAPTIVE_EXCHANGE" then
                local marker=Game:GetProperty("JAC_BOUNDARY_CAPTIVE_DEAL_ATTEMPT");
                accepted=marker and marker.token==request.token and
                    marker.spyID==request.payload.SpyID and
                    marker.eventID==request.payload.EventID and
                    marker.captor==request.payload.Captor;
            elseif request.action=="PREPARE_DISTRICT_CIVIC" then
                local marker=Game:GetProperty("JAC_BOUNDARY_CAPACITY_CIVIC");
                local civic=GameInfo.Civics["CIVIC_JAC_SENGOKU_DOMAIN_LAW"];
                accepted=marker and civic and marker.token==request.token and
                    marker.index==civic.Index and
                    Players[Game.GetLocalPlayer()]:GetCulture():HasCivic(civic.Index);
            elseif request.action=="PROGRESS_CAPACITY_DISTRICT" then
                local marker=Game:GetProperty("JAC_BOUNDARY_CAPACITY_DISTRICT_PROGRESS");
                accepted=marker and marker.token==request.token and
                    marker.cityID==request.payload.CityID and
                    marker.plot==request.payload.PlotIndex and
                    marker.districtIndex==request.payload.DistrictIndex;
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
        local action=request.action or "BUILD_SPY";
        local check=action=="BUILD_CAPACITY" and CapacityBuildReady or
            action=="BUILD_CAPACITY_DISTRICT" and CapacityDistrictBuildReady or SpyBuildReady;
        local ok,ready=pcall(check,request.owner,request.cityID,
            request.buildingType or request.districtType);
        if action=="BUILD_CAPACITY" and ok and ready then
            capacityBuildReady={cityID=request.cityID,buildingType=request.buildingType,
                turn=Game.GetCurrentGameTurn()};
        end
        if action=="BUILD_CAPACITY_DISTRICT" and ok and ready then
            capacityDistrictReady={cityID=request.cityID,plot=request.plot,
                x=request.x,y=request.y,turn=Game.GetCurrentGameTurn()};
        end
        B.lastAction=(ok and ready and "READY_" or "REJECTED_")..action;
        Log(action=="BUILD_CAPACITY_DISTRICT" and "CAPACITY_DISTRICT_BUILD_ACCEPTANCE" or
            action=="BUILD_CAPACITY" and "CAPACITY_BUILD_ACCEPTANCE" or "SPY_BUILD_ACCEPTANCE",
            {ok=ok,ready=ok and ready or false,error=not ok and tostring(ready) or "",
             cityID=request.cityID,buildingType=request.buildingType,
             districtType=request.districtType,plot=request.plot});
        return;
    end
    if not pending then return; end
    local command=pending; pending=nil;
    local ok, result=pcall(Submit, command);
    if not ok and (command.action=="BUILD_SPY" or command.action=="BUILD_CAPACITY" or
        command.action=="BUILD_CAPACITY_DISTRICT") then
        buildPending=nil;
    end
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
Events.DiplomacyIncomingDeal.Add(function(fromPlayer,toPlayer,action)
    local x=captiveExchange;
    if x and x.proposed and fromPlayer==x.captor and toPlayer==x.attacker then
        x.response=action;
        Log("ENEMY_CAPTIVE_EXCHANGE_AI_RESPONSE",{from=fromPlayer,to=toPlayer,
            action=action,finalSent=x.finalSent});
    end
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
    elseif verification and verification.kind~="ENEMY_SYNC" and
        verification.stage=="BARRIER_SENT" then
        local marker=Game:GetProperty("JAC_BOUNDARY_SPY_BARRIER");
        if marker and marker.owner==verification.before.owner and marker.spyID==verification.before.id and
            marker.token==verification.token and marker.action==verification.action then
            verification.published=true;
        end
    end
    Observe("PUBLISH");
end);
Events.LocalPlayerTurnBegin.Add(function() Observe("TURN"); end);
Events.LoadScreenClose.Add(function()
    emptyRosterObservation=nil;
    last=nil;
    Observe("LOAD");
end);
Events.SpyMissionCompleted.Add(function(attacker, missionID)
    -- The event's player is the attacker, not the governor owner.
    Log("MISSION_EVENT", {attacker=attacker, mission=missionID, turn=Game.GetCurrentGameTurn()});
    local c=Game:GetProperty("JAC_BOUNDARY_ENEMY_CONFIG");
    if c and attacker==c.attacker and c.cityID then
        local retry=Game:GetProperty(RETRY);
        if retry then
            if missionID==retry.archive.eventID then
                Log("ARCHIVED_MISSION_EVENT_IGNORED",{attacker=attacker,id=missionID});
                return;
            end
            local marker=Game:GetProperty("JAC_BOUNDARY_SPY_BARRIER");
            assert(retry.missionRequested and marker and marker.action=="SPY_MISSION" and
                marker.spyID==retry.spyID,
                "retry completion has no matching official mission request");
        end
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
