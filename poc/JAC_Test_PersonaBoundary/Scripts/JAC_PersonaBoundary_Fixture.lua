-- Opt-in test MOD only. Mutations run through synchronized EXECUTE_SCRIPT.
-- Primary API evidence and unresolved spy control: documents/poc/pr86-boundary-diagnostic.md.
include("JAC_PersonaBoundaryRules");
local R = JAC_PersonaBoundaryRules;
local LEADER = "LEADER_JAC_ODA_NOBUHIDE";
local CONFIG = "JAC_BOUNDARY_CONFIG";
local ENEMY_CONFIG = "JAC_BOUNDARY_ENEMY_CONFIG";
local ENEMY_LEADER = "LEADER_JAC_ASHINA_MORIKIYO";
local SPY_CIVIC = "CIVIC_JAC_SENGOKU_LOCAL_LEAGUES_AND_INTELLIGENCE";
local RETRY = "JAC_BOUNDARY_ENEMY_SPY_RETRY";
local PRODUCTION = "JAC_BOUNDARY_SPY_PRODUCTION";
local GAMEPLAY_API_PROBE = "JAC_BOUNDARY_GAMEPLAY_API_PROBE";

-- This callback lives in AddGameplayScripts. A FireTuner GameCore state has a
-- different lifetime and can expose methods unavailable to the gameplay VM.
-- Inspect published members only; never call a candidate mutator.
GameEvents.JAC_BoundaryGameplayApiProbe.Add(function(owner)
    local player = assert(Players[owner], "probe owner missing");
    assert(player:IsHuman() and player:IsAlive(), "probe requires its living human controller");
    local function member_type(object, key)
        local ok, value = pcall(function() return object and object[key]; end);
        return ok and type(value) or "lookup_error";
    end
    local function raw_member_type(object, key)
        if type(object) ~= "table" then return "not_table"; end
        -- Persistent Gameplay omits rawget in the real game (live 88).
        -- Lua 5.1 pairs enumerates stored entries without synthesizing __index.
        for stored_key,value in pairs(object) do
            if stored_key==key then return type(value); end
        end
        return "nil";
    end
    local function metadata(object)
        local ok, mt = pcall(getmetatable, object);
        if not ok then return "lookup_error"; end
        if type(mt) ~= "table" then return type(mt); end
        local keys = {};
        for key in pairs(mt) do
            if type(key) == "string" then keys[#keys + 1] = key; end
        end
        table.sort(keys);
        if #keys > 80 then
            local count = #keys;
            while #keys > 80 do table.remove(keys); end
            return table.concat(keys, ",") .. ",.../" .. tostring(count);
        end
        return table.concat(keys, ",");
    end
    local sections = {"source=persistent_gameplay", "owner=" .. tostring(owner)};
    local function inspect(label, object, names, raw_only)
        sections[#sections + 1] = label .. ".type=" .. type(object);
        sections[#sections + 1] = label .. ".metatable=" .. metadata(object);
        for _, name in ipairs(names) do
            local kind = raw_only and raw_member_type(object, name) or member_type(object, name);
            sections[#sections + 1] = label .. "." .. name .. "=" .. kind;
        end
    end
    local governors = player:GetGovernors(); -- existing read-only Gameplay API
    inspect("governors", governors, {"GetGovernorPoints", "GetGovernorPointsSpent",
        "Appoint", "AppointGovernor", "Promote", "PromoteGovernor", "Assign", "AssignGovernor",
        "CanEverAppointGovernor", "CanAppointGovernor", "GetGovernor", "HasGovernor"});
    inspect("player", player, {"GetTraits", "GrantTrait", "AddTrait", "SetTrait",
        "AttachModifierByID", "GetProperty"});
    inspect("game", Game, {"RequestPlayerOperation", "RequestOperation", "GetProperty"});
    -- Event registries may synthesize unknown keys through __index. rawget
    -- reports only published table entries and never creates a fake event.
    inspect("game_events_raw", GameEvents, {"PlayerTurnStarted", "PlayerTurnStartComplete",
        "GovernorAppointed", "GovernorPromoted", "GovernorAssigned", "RequestPlayerOperation"}, true);
    inspect("events_raw", Events, {"GovernorChanged", "GovernorPromoted", "GovernorAssigned",
        "GovernorEstablished", "GovernorPointsChanged"}, true);
    inspect("governor_manager", GovernorManager, {"Appoint", "Promote", "Assign"});
    inspect("ui", UI, {"RequestPlayerOperation"});
    inspect("player_operations", PlayerOperations, {"APPOINT_GOVERNOR", "PROMOTE_GOVERNOR",
        "ASSIGN_GOVERNOR", "EXECUTE_SCRIPT"});
    local result = table.concat(sections, "|");
    Game:SetProperty(GAMEPLAY_API_PROBE, result);
    print("JAC_BOUNDARY_GAMEPLAY_API_PROBE " .. result);
end);
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
    assert(not Game:GetProperty(PRODUCTION) and not Game:GetProperty("JAC_BOUNDARY_SPY"),
        "fixture spy production already started");
    if c.attacker then
        local retry=assert(Game:GetProperty(RETRY),"enemy retry was not archived");
        assert(retry.mode=="PRODUCE" and retry.current==2 and retry.archive and
            not retry.spyID,"enemy retry does not permit another spy production");
    end
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
    Game:SetProperty(PRODUCTION,{owner=owner,cityID=capital:GetID(),progressed=false});
    Game:SetProperty("JAC_BOUNDARY_SPY_PREPARED", {owner=owner, cityID=capital:GetID()});
    print("JAC_BOUNDARY SPY_CIVIC_READY " .. R.Serialize(Game:GetProperty("JAC_BOUNDARY_SPY_PREPARED")));
end);

GameEvents.JAC_BoundaryProgressSpy.Add(function(owner)
    Config(owner);
    local production=assert(Game:GetProperty(PRODUCTION),"spy production preparation missing");
    assert(production.owner == owner and not production.progressed,
        "spy production progress was not prepared or was already added");
    local city = assert(CityManager.GetCity(owner, production.cityID), "spy production city missing");
    local queue = city:GetBuildQueue();
    -- The visible UI already independently observed the accepted native queue.
    -- GetAt is UI-only in live 50; Gameplay performs the mutation and binds the
    -- actual produced unit afterwards, never treating its request as completion.
    local unit = assert(GameInfo.Units["UNIT_SPY"], "native spy definition missing");
    assert(type(queue.AddProgress) == "function", "native Gameplay production API unavailable");
    local before = {};
    local count = 0;
    for _,candidate in Players[owner]:GetUnits():Members() do
        if candidate:GetType() == unit.Index then before[#before+1]=candidate:GetID(); count = count + 1; end
    end
    production.before=before;
    production.beforeCount=count;
    production.progressed=true; -- one AddProgress even if its result is unknown
    Game:SetProperty(PRODUCTION,production);
    -- Official Debug/City.ltp:200-204. The native queue, not a spy Property, creates the unit.
    queue:AddProgress(500);
    Game:SetProperty("JAC_BOUNDARY_SPY_PROGRESS", {owner=owner, cityID=city:GetID(), beforeCount=count});
    print("JAC_BOUNDARY SPY_PRODUCTION_PROGRESS " .. R.Serialize(Game:GetProperty("JAC_BOUNDARY_SPY_PROGRESS")));
end);

GameEvents.JAC_BoundaryBindSpy.Add(function(owner)
    Config(owner);
    local production=assert(Game:GetProperty(PRODUCTION),"spy production preparation missing");
    assert(production.owner == owner and production.progressed and
        not Game:GetProperty("JAC_BOUNDARY_SPY"), "spy production was not completed once");
    local before={};
    for _,id in ipairs(production.before) do before[id]=true; end
    local spyType = GameInfo.Units["UNIT_SPY"].Index;
    local spy, count = nil, 0;
    for _,unit in Players[owner]:GetUnits():Members() do
        if unit:GetType() == spyType then
            count = count + 1;
            if not before[unit:GetID()] then
                assert(not spy, "native production added multiple spies"); spy = unit;
            end
        end
    end
    assert(spy and count == production.beforeCount + 1, "native spy production did not add exactly one unit");
    assert(spy:GetOwner() == owner, "produced native spy owner mismatch");
    local retry=Game:GetProperty(RETRY);
    if retry then
        assert(retry.mode=="PRODUCE" and retry.current==2 and not retry.spyID and
            spy:GetID()~=retry.archive.spyID,"retry production did not bind a distinct native spy");
        retry.spyID=spy:GetID();
        Game:SetProperty(RETRY,retry);
    end
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
    local retry=Game:GetProperty(RETRY);
    if retry then
        assert(retry.current==2 and retry.spyID==spy.id,
            "retry barrier belongs to the archived spy");
        if params.Action=="SPY_MISSION" then
            assert(not retry.missionRequested,
                "a second official mission request is forbidden in one retry");
            retry.missionRequested=true;
            Game:SetProperty(RETRY,retry);
        end
    end
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
    local retry=Game:GetProperty(RETRY);
    if retry then
        assert(retry.current==2 and retry.spyID==params.OldID,
            "new native spy identity does not follow the current retry");
        retry.spyID=spy:GetID();
        if retry.mode=="RETURN" then retry.mode="MAP"; end
        Game:SetProperty(RETRY,retry);
    end
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

-- The UI owns Governor eligibility and instance methods. Gameplay only reads
-- official title accounting, the product's existing UI native-read bridge and
-- its own city/person ledger. Never call UI-only Governor getters here.
local function NativeEnemy(c)
    local read=assert(ExposedMembers.JAC_LeaderPersonaNativeRead,
        "product UI native Governor reader unavailable");
    local value=assert(read(c.owner,c.leader),"enemy native Governor observation unavailable");
    assert(type(value.appointed)=="boolean" and type(value.canAppoint)=="boolean",
        "enemy native Governor observation incomplete");
    return value;
end
local function EnemyReceipt(controller,params,action)
    EnemyController(controller);
    assert(type(params)=="table" and params.Action==action and
        type(params.Token)=="number" and params.Token>0 and
        type(params.Turn)=="number" and params.Turn==Game.GetCurrentGameTurn() and
        type(params.Frame)=="number" and params.Frame>0 and
        type(params.PublishEpoch)=="number" and params.PublishEpoch>=0 and
        params.Attacker==controller and params.Defender==1 and
        params.Source==2 and params.Leader==ENEMY_LEADER,
        "enemy UI observation is stale or belongs to another actor");
end

-- A separate fixture keeps the earlier human Oda/city-loss experiment intact.
-- UI checks the static candidate gate before this first synchronized write.
-- No AI title is granted before the regular Join.
GameEvents.JAC_BoundaryPrepareEnemy.Add(function(controller,params)
    EnemyReceipt(controller,params,"PREPARE_ENEMY");
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
    assert(params.GovernorIndex==def.Index and params.GovernorHash==def.Hash,
        "enemy Governor definition changed after UI qualification");
    local governors=defender:GetGovernors();
    local total,spent=governors:GetGovernorPoints(),governors:GetGovernorPointsSpent();
    assert(total==params.Total and spent==params.Spent,
        "official AI title accounting changed after UI qualification");
    assert(not NativeEnemy({owner=1,leader=ENEMY_LEADER}).appointed,
        "unjoined enemy Governor already appointed");
    local seam=assert(ExposedMembers.JAC_LeaderPersona, "regular persona seam missing");
    assert(seam.Join(1,2)=="JOINED", "regular enemy Join failed");
    assert(R.Property(ENEMY_LEADER,"OWNER")==1 and R.Property(ENEMY_LEADER,"SOURCE")==2 and
        R.Property(ENEMY_LEADER,"STATE")=="GREAT_PERSON", "joined enemy identity mismatch");
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
    Game:SetProperty(ENEMY_CONFIG,{attacker=controller,owner=1,source=2,leader=ENEMY_LEADER,
        skill=ENEMY_LEADER.."_SIGNATURE_1",total=governors:GetGovernorPoints(),
        spent=governors:GetGovernorPointsSpent(),armedTurn=Game.GetCurrentGameTurn(),
        preflightToken=params.Token,preflightFrame=params.Frame,
        preflightEpoch=params.PublishEpoch,preparedTurn=Game.GetCurrentGameTurn(),
        controlRequired=false,requireSkill=false});
    print("JAC_BOUNDARY ENEMY_PREPARED "..R.Serialize(Game:GetProperty(ENEMY_CONFIG)));
end);

-- The first request's publish barrier gives the UI title cache time to refresh.
-- This still runs on human turn 0, before the defender's first native AI turn.
GameEvents.JAC_BoundaryEvolveEnemy.Add(function(controller,params)
    EnemyReceipt(controller,params,"EVOLVE_ENEMY");
    local c=EnemyConfig(controller);
    assert(c.preflightToken and not c.evolvedToken and
        params.Token~=c.preflightToken and params.Frame>c.preflightFrame and
        params.PublishEpoch>c.preflightEpoch and
        c.preparedTurn==Game.GetCurrentGameTurn() and
        R.Property(c.leader,"OWNER")==c.owner and
        R.Property(c.leader,"STATE")=="GREAT_PERSON",
        "enemy Join must complete before evolution preparation");
    local gs=Players[c.owner]:GetGovernors();
    assert(gs:GetGovernorPoints()==c.total and gs:GetGovernorPointsSpent()==c.spent and
        params.Total==c.total and params.Spent==c.spent,
        "official cost-one title budget changed before evolution");
    local native=NativeEnemy(c);
    assert(not native.appointed and native.canAppoint,
        "product native reader does not allow the joined cost-one person");
    local result=ExposedMembers.JAC_LeaderPersona.PrepareEvolution(c.owner,c.leader);
    assert(result=="EVOLVING" and R.Property(c.leader,"STATE")=="EVOLVING",
        "regular enemy evolution preparation failed");
    c.evolvedToken=params.Token;
    Game:SetProperty(ENEMY_CONFIG,c);
end);

GameEvents.JAC_BoundaryEnemyFixtureBarrier.Add(function(controller,params)
    EnemyReceipt(controller,params,params and params.Action);
    assert(params.Action=="PREPARE_ENEMY" or params.Action=="EVOLVE_ENEMY" or
        params.Action=="CONFIRM_ENEMY" or params.Action=="CAPTURE_ENEMY" or
        params.Action=="BEGIN_SPY_RETRY" or params.Action=="PROGRESS_CAPACITY" or
        params.Action=="MARK_CAPTIVE_EXCHANGE" or
        params.Action=="PREPARE_DISTRICT_CIVIC" or
        params.Action=="PROGRESS_CAPACITY_DISTRICT",
        "unknown enemy fixture barrier");
    Game:SetProperty("JAC_BOUNDARY_ENEMY_FIXTURE_BARRIER",
        {action=params.Action,token=params.Token,turn=params.Turn,attacker=controller});
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

GameEvents.JAC_BoundaryConfirmEnemy.Add(function(controller,params)
    EnemyReceipt(controller,params,"CONFIRM_ENEMY");
    local c=EnemyConfig(controller);
    local state=R.Property(c.leader,"STATE");
    assert(c.evolvedToken and not c.cityID and
        (state=="EVOLVING" or state=="GOVERNOR"),
        "regular enemy evolution required before native binding");
    local native=NativeEnemy(c);
    assert(native.appointed and native.base and native.surchargePaid==0 and
        native.cityOwner==c.owner and native.cityID==params.CityID,
        "native UI observation does not match the joined Governor and city");
    local gs=Players[c.owner]:GetGovernors();
    assert(gs:GetGovernorPoints()>=c.total and gs:GetGovernorPointsSpent()==c.spent+1 and
        params.Total==gs:GetGovernorPoints() and params.Spent==gs:GetGovernorPointsSpent(),
        "official cost-one Governor appointment or native accounting missing");
    local city=assert(CityManager.GetCity(c.owner,params.CityID),
        "enemy native assignment city missing in Gameplay");
    assert(city:GetOwner()==c.owner and city:GetX()==params.X and city:GetY()==params.Y,
        "UI/Gameplay enemy assignment city identity disagrees");
    local seam=assert(ExposedMembers.JAC_LeaderPersona,"regular persona seam missing");
    local result=seam.CompleteEvolution(c.owner,c.leader);
    assert((result=="EVOLVED" or result=="ALREADY_EVOLVED") and
        R.Property(c.leader,"STATE")=="GOVERNOR", "regular evolution did not remove the GP");
    c.cityID=city:GetID(); c.x=city:GetX(); c.y=city:GetY();
    c.armedTurn=Game.GetCurrentGameTurn();
    Game:SetProperty(ENEMY_CONFIG,c);
    print("JAC_BOUNDARY ENEMY_NATIVE_BOUND "..R.Serialize(c));
end);

GameEvents.JAC_BoundaryRecordEnemyMission.Add(function(controller,params)
    local c=EnemyConfig(controller);
    local retry=Game:GetProperty(RETRY);
    if retry then
        assert(retry.current==2 and retry.spyID and params.MissionID~=retry.archive.eventID,
            "archived failed native mission cannot be recorded for the retry");
    end
    assert(c.cityID and type(params)=="table" and params.Attacker==c.attacker and
        type(params.MissionID)=="number" and params.MissionID>=0 and
        not Game:GetProperty("JAC_BOUNDARY_ENEMY_MISSION"), "invalid or repeated enemy mission event");
    Game:SetProperty("JAC_BOUNDARY_ENEMY_MISSION",{attacker=params.Attacker,
        id=params.MissionID,turn=Game.GetCurrentGameTurn()});
end);

-- Archive the verified failed attempt in test-only save data before replacing
-- any fixture pointers. Civ6's unit, mission history, Governor and counters are untouched.
GameEvents.JAC_BoundaryBeginEnemySpyRetry.Add(function(controller,params)
    EnemyReceipt(controller,params,"BEGIN_SPY_RETRY");
    local c=EnemyConfig(controller);
    assert(c.cityID and type(params)=="table" and params.Action=="BEGIN_SPY_RETRY" and
        params.Mode and (params.Mode=="PRODUCE" or params.Mode=="RETURN" or
        params.Mode=="MAP") and not Game:GetProperty(RETRY),
        "enemy retry must begin once from the preserved failed attempt");
    local event=assert(Game:GetProperty("JAC_BOUNDARY_ENEMY_MISSION"),
        "failed native mission event missing");
    local spy=assert(Game:GetProperty("JAC_BOUNDARY_SPY"),"failed native spy reference missing");
    local travel=assert(Game:GetProperty("JAC_BOUNDARY_SPY_TRAVEL"),
        "accepted original travel receipt missing");
    local barrier=assert(Game:GetProperty("JAC_BOUNDARY_SPY_BARRIER"),
        "official original mission barrier missing");
    local native=NativeEnemy(c);
    local points=Players[c.owner]:GetGovernors();
    assert(event.attacker==controller and event.id==params.EventID and
        event.turn==params.EventTurn and spy.owner==controller and spy.id==params.SpyID and
        travel.targetOwner==c.owner and travel.targetID==c.cityID and
        barrier.action=="SPY_MISSION" and barrier.spyID==spy.id and
        native.appointed and native.base and native.neutralized==0 and
        native.cityOwner==c.owner and native.cityID==c.cityID and
        points:GetGovernorPointsSpent()==params.Spent and
        points:GetGovernorPoints()==params.Total and
        type(params.Initial)=="number" and
        params.Plot==Map.GetPlot(c.x,c.y):GetIndex() and
        params.CompletionTurn==event.turn and
        params.GovernorType==GameInfo.Governors[
            GameInfo.JAC_LeaderPersonas[c.leader].GovernorType].Index and
        GameInfo.UnitOperations[params.Operation] and
        GameInfo.UnitOperations[params.Operation].Hash==GameInfo.UnitOperations[
            "UNITOPERATION_SPY_NEUTRALIZE_GOVERNOR"].Hash,
        "failed native history, Governor or official title receipt changed");
    if params.Mode=="PRODUCE" then
        assert(not Players[controller]:GetUnits():FindID(spy.id),
            "old native spy is still on the map; production retry is invalid");
    elseif params.Mode=="MAP" then
        local current=assert(Players[controller]:GetUnits():FindID(params.CurrentID),
            "living native map spy missing at archive");
        assert(current:GetOwner()==controller and
            current:GetType()==GameInfo.Units["UNIT_SPY"].Index,
            "living native map spy identity differs at archive");
    end
    local archive={spyID=spy.id,eventID=event.id,eventTurn=event.turn,
        operation=params.Operation,plot=params.Plot,initial=params.Initial,
        escape=params.Escape,completionTurn=params.CompletionTurn,
        travelOldID=travel.oldID,travelName=travel.name,travelLevel=travel.level,
        targetOwner=travel.targetOwner,targetID=travel.targetID,
        governorType=params.GovernorType,governorNeutralized=native.neutralized,
        governorCityID=native.cityID,spent=params.Spent,total=params.Total,
        merit=R.Property(c.leader,"MERIT") or 0,rosterOutcome=params.Outcome,
        captiveCaptor=params.CaptiveCaptor,captiveName=params.CaptiveName,
        captiveNameIndex=params.CaptiveNameIndex,captiveLevel=params.CaptiveLevel,
        capacity=params.Capacity,occupied=params.Occupied,archivedTurn=Game.GetCurrentGameTurn()};
    local retry={current=2,mode=params.Mode,archive=archive,
        spyID=spy.id,
        returnName=params.ReturnName,returnLevel=params.ReturnLevel,
        returnTurn=params.ReturnTurn,returnX=params.ReturnX,returnY=params.ReturnY};
    if params.Mode=="PRODUCE" then retry.spyID=nil; end
    if params.Mode=="MAP" then retry.spyID=params.CurrentID; end
    Game:SetProperty(RETRY,retry);
    Game:SetProperty("JAC_BOUNDARY_ENEMY_MISSION",nil);
    Game:SetProperty("JAC_BOUNDARY_SPY_BARRIER",nil);
    Game:SetProperty("JAC_BOUNDARY_SPY_TRAVEL",nil);
    if params.Mode=="PRODUCE" then
        Game:SetProperty("JAC_BOUNDARY_SPY",nil);
        Game:SetProperty("JAC_BOUNDARY_SPY_PREPARED",nil);
        Game:SetProperty("JAC_BOUNDARY_SPY_PROGRESS",nil);
        Game:SetProperty(PRODUCTION,nil);
    elseif params.Mode=="MAP" and params.CurrentID~=spy.id then
        Game:SetProperty("JAC_BOUNDARY_SPY",{owner=controller,id=params.CurrentID});
    end
    print("JAC_BOUNDARY ENEMY_SPY_RETRY_ARCHIVED "..R.Serialize(retry));
end);

-- One-time test receipt for a normal native diplomacy attempt. This neither
-- changes the captive roster nor claims that the AI accepted a trade.
GameEvents.JAC_BoundaryMarkCaptiveExchange.Add(function(controller,params)
    EnemyReceipt(controller,params,"MARK_CAPTIVE_EXCHANGE");
    local c=EnemyConfig(controller);
    local event=assert(Game:GetProperty("JAC_BOUNDARY_ENEMY_MISSION"),
        "failed native mission event missing before captive exchange");
    local spy=assert(Game:GetProperty("JAC_BOUNDARY_SPY"),
        "old native captive spy reference missing");
    local travel=assert(Game:GetProperty("JAC_BOUNDARY_SPY_TRAVEL"),
        "original native spy travel missing");
    assert(not Game:GetProperty("JAC_BOUNDARY_CAPTIVE_DEAL_ATTEMPT") and
        not Game:GetProperty(RETRY) and event.attacker==controller and
        event.id==params.EventID and event.turn==params.EventTurn and
        spy.owner==controller and spy.id==params.SpyID and
        params.Captor==c.owner and type(params.NameIndex)=="number" and
        travel.targetOwner==c.owner and travel.targetID==c.cityID,
        "native captive exchange receipt changed or was repeated");
    local native=NativeEnemy(c);
    assert(native.appointed and native.base and native.neutralized==0 and
        native.cityOwner==c.owner and native.cityID==c.cityID,
        "native enemy Governor changed before captive exchange");
    Game:SetProperty("JAC_BOUNDARY_CAPTIVE_DEAL_ATTEMPT",{
        attacker=controller,captor=params.Captor,nameIndex=params.NameIndex,
        spyID=params.SpyID,eventID=params.EventID,eventTurn=params.EventTurn,
        token=params.Token,turn=Game.GetCurrentGameTurn()});
end);

GameEvents.JAC_BoundaryPrepareCapacityCivic.Add(function(controller,params)
    EnemyReceipt(controller,params,"PREPARE_DISTRICT_CIVIC");
    local c=EnemyConfig(controller);
    assert(c.cityID and not Game:GetProperty(RETRY) and
        not Game:GetProperty("JAC_BOUNDARY_CAPACITY_CIVIC"),
        "native capacity Civic was already prepared");
    local civic=assert(GameInfo.Civics["CIVIC_JAC_SENGOKU_DOMAIN_LAW"],
        "native Government Plaza Civic missing");
    local player=assert(Players[controller],"capacity Civic owner missing");
    local culture=player:GetCulture();
    local titles=player:GetGovernors();
    assert(params.CivicIndex==civic.Index and not culture:HasCivic(civic.Index) and
        titles:GetGovernorPoints()==params.Total and
        titles:GetGovernorPointsSpent()==params.Spent,
        "native capacity Civic or title accounting changed");
    assert(type(culture.SetCivic)=="function","native civic research API missing");
    culture:SetCivic(civic.Index,true);
    assert(culture:HasCivic(civic.Index) and
        titles:GetGovernorPointsSpent()==params.Spent,
        "normal native Civic research or title accounting failed");
    Game:SetProperty("JAC_BOUNDARY_CAPACITY_CIVIC",{
        owner=controller,index=civic.Index,token=params.Token,
        beforeTotal=params.Total,beforeSpent=params.Spent,
        afterTotal=titles:GetGovernorPoints(),
        afterSpent=titles:GetGovernorPointsSpent(),turn=Game.GetCurrentGameTurn()});
end);

GameEvents.JAC_BoundaryProgressCapacityDistrict.Add(function(controller,params)
    EnemyReceipt(controller,params,"PROGRESS_CAPACITY_DISTRICT");
    local c=EnemyConfig(controller);
    assert(c.cityID and not Game:GetProperty(RETRY) and
        not Game:GetProperty("JAC_BOUNDARY_CAPACITY_DISTRICT_PROGRESS") and
        params.Capacity==1 and params.Occupied==1 and
        params.DistrictType=="DISTRICT_GOVERNMENT" and
        type(params.CityID)=="number" and type(params.PlotIndex)=="number" and
        type(params.X)=="number" and type(params.Y)=="number" and
        type(params.MemberKey)=="number" and
        type(params.DistrictID)=="number" and params.DistrictID>=0 and
        type(params.AcceptedNonce)=="number" and params.AcceptedNonce>0,
        "native Government Plaza progress receipt is stale");
    local district=assert(GameInfo.Districts[params.DistrictType],
        "native Government Plaza definition missing");
    local plot=assert(Map.GetPlot(params.X,params.Y),
        "native Government Plaza placement plot missing");
    local city=assert(CityManager.GetCity(controller,params.CityID),
        "native Government Plaza city missing");
    local purchase=Cities.GetPlotPurchaseCity(plot);
    assert(district.Index==params.DistrictIndex and
        plot:GetIndex()==params.PlotIndex and
        city:GetOwner()==controller and purchase and
        purchase:GetOwner()==controller and purchase:GetID()==city:GetID(),
        "native Government Plaza owner or plot changed");
    local read=assert(ExposedMembers.JAC_PersonaBoundaryCapacityQueueRead,
        "UI native Government Plaza queue reader unavailable");
    local current=assert(read(controller,params.CityID),
        "UI native Government Plaza queue observation unavailable");
    assert(current.owner==controller and current.cityID==params.CityID and
        current.districtType==params.DistrictType and current.turn==params.Turn and
        current.districtOwner==controller and
        current.districtCityOwner==controller and
        current.districtCityID==params.CityID and
        current.districtID==params.DistrictID and
        current.districtPlot==params.PlotIndex and
        current.districtX==params.X and current.districtY==params.Y and
        current.memberKey==params.MemberKey and
        current.acceptedNonce==params.AcceptedNonce and
        type(current.frame)=="number" and current.frame>=params.Frame and
        type(current.publishEpoch)=="number" and
        current.publishEpoch>=params.PublishEpoch,
        "native Government Plaza queue changed before progress");
    local queue=city:GetBuildQueue();
    assert(type(queue.AddProgress)=="function","native district progress API unavailable");
    queue:AddProgress(500);
    Game:SetProperty("JAC_BOUNDARY_CAPACITY_DISTRICT_PROGRESS",{
        owner=controller,cityOwner=current.districtCityOwner,
        cityID=city:GetID(),districtIndex=district.Index,
        districtID=current.districtID,
        plot=plot:GetIndex(),x=plot:GetX(),y=plot:GetY(),
        memberKey=params.MemberKey,acceptedNonce=params.AcceptedNonce,
        token=params.Token,turn=Game.GetCurrentGameTurn()});
end);

-- The UI first verifies a normal Government Plaza building BUILD request and
-- sends only scalar identity through the synchronized Gameplay operation.
-- Progress is applied once to the real city queue. Only a later UI building
-- observation and native GetSpyCapacity can establish that a slot was earned.
GameEvents.JAC_BoundaryProgressCapacityBuilding.Add(function(controller,params)
    EnemyReceipt(controller,params,"PROGRESS_CAPACITY");
    local c=EnemyConfig(controller);
    assert(c.cityID and not Game:GetProperty(RETRY) and
        params.Capacity==1 and params.Occupied==1 and
        type(params.CityID)=="number" and type(params.BuildingIndex)=="number" and
        type(params.BuildingType)=="string",
        "native captive capacity progress receipt is stale");
    local row=assert(GameInfo.Buildings[params.BuildingType],
        "official capacity building definition missing");
    assert(row.Index==params.BuildingIndex and
        row.PrereqDistrict=="DISTRICT_GOVERNMENT" and
        (row.BuildingType=="BUILDING_GOV_TALL" or
         row.BuildingType=="BUILDING_GOV_CONQUEST" or
         row.BuildingType=="BUILDING_GOV_WIDE" or
         row.BuildingType=="BUILDING_GOV_SPIES"),
        "capacity progress is not an official Government Plaza building");
    local city=assert(CityManager.GetCity(controller,params.CityID),
        "owned capacity production city missing");
    assert(city:GetOwner()==controller,"capacity production city owner changed");
    local progress=Game:GetProperty("JAC_BOUNDARY_CAPACITY_PROGRESS") or {steps={}};
    assert(not progress.steps[row.BuildingType],
        "capacity building progress was already added once");
    local read=assert(ExposedMembers.JAC_PersonaBoundaryCapacityQueueRead,
        "UI native capacity queue reader unavailable");
    local current=assert(read(controller,params.CityID),
        "UI native capacity queue observation unavailable");
    assert(type(current)=="table" and current.owner==controller and
        current.cityID==params.CityID and current.buildingType==row.BuildingType and
        current.turn==params.Turn and type(current.frame)=="number" and
        current.frame>=params.Frame and type(current.publishEpoch)=="number" and
        current.publishEpoch>=params.PublishEpoch,
        "native capacity building queue changed before progress");
    local queue=city:GetBuildQueue();
    assert(type(queue.AddProgress)=="function","native city progress API missing");
    queue:AddProgress(500);
    progress.steps[row.BuildingType]={token=params.Token,cityID=params.CityID,
        buildingIndex=row.Index,turn=Game.GetCurrentGameTurn()};
    progress.latest={buildingType=row.BuildingType,cityID=params.CityID,
        buildingIndex=row.Index,token=params.Token,turn=Game.GetCurrentGameTurn()};
    Game:SetProperty("JAC_BOUNDARY_CAPACITY_PROGRESS",progress);
    print("JAC_BOUNDARY CAPACITY_BUILDING_PROGRESS "..R.Serialize(progress));
end);

GameEvents.JAC_BoundaryCaptureEnemyRecovery.Add(function(controller,params)
    EnemyReceipt(controller,params,"CAPTURE_ENEMY");
    local c=EnemyConfig(controller);
    assert(c.cityID and not Game:GetProperty("JAC_BOUNDARY_ENEMY_RECOVERY"),
        "recovery baseline missing target or already recorded");
    local native=NativeEnemy(c);
    local remaining=native.neutralized;
    assert(type(remaining)=="number" and remaining>0,"native neutralization required for recovery");
    assert(remaining==params.Neutralized and params.CityID==c.cityID,
        "UI/Gameplay neutralization observation changed before capture");
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
