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
local RETRY_PENDING = "JAC_BOUNDARY_ENEMY_SPY_RETRY_PENDING";
local THIRD_CIVIC_INTENT = "JAC_BOUNDARY_THIRD_CIVIC_INTENT";
local PRODUCTION = "JAC_BOUNDARY_SPY_PRODUCTION";
local GAMEPLAY_API_PROBE = "JAC_BOUNDARY_GAMEPLAY_API_PROBE";
local CAPACITY_CIVIC_RECEIPT = "JAC_BOUNDARY_CAPACITY_CIVIC_RECEIPT";
local CAPACITY_CIVIC_ATTEMPT = "JAC_BOUNDARY_CAPACITY_CIVIC_ATTEMPT";
local CAPACITY_CIVIC_READ = "JAC_BOUNDARY_CAPACITY_CIVIC_READ";
local TIER2_CIVIC_ATTEMPT = "JAC_BOUNDARY_TIER2_CIVIC_ATTEMPT";
local TIER2_CIVIC_RECEIPT = "JAC_BOUNDARY_TIER2_CIVIC_RECEIPT";
local TIER2_CIVIC_READ = "JAC_BOUNDARY_TIER2_CIVIC_READ";
local TIER2_CIVIC_MARKER = "JAC_BOUNDARY_TIER2_CIVIC";
local GOV_WINDOW_ATTEMPT = "JAC_BOUNDARY_GOV_WINDOW_CIVIC_ATTEMPT";
local GOV_WINDOW_RECEIPT = "JAC_BOUNDARY_GOV_WINDOW_CIVIC_RECEIPT";
local GOV_WINDOW_READ = "JAC_BOUNDARY_GOV_WINDOW_CIVIC_READ";
local GOV_WINDOW_MARKER = "JAC_BOUNDARY_GOV_WINDOW_CIVIC";
local CAPACITY_GOVERNMENT_INTENTS = "JAC_BOUNDARY_CAPACITY_GOVERNMENT_INTENTS";
local CAPACITY_GOVERNMENT_READ = "JAC_BOUNDARY_CAPACITY_GOVERNMENT_READ";
local CAPACITY_GOVERNMENT_API_PROBE = "JAC_BOUNDARY_CAPACITY_GOVERNMENT_API_PROBE";
local GOV_CLASSIFY_INTENT = "JAC_BOUNDARY_GOV_CLASSIFY_INTENT";
local GOV_CLASSIFY_READ = "JAC_BOUNDARY_GOV_CLASSIFY_READ";

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
    if R.Third and R.Third.IsThird(Game:GetProperty(RETRY)) then R.Third.Progress(owner); return; end
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
        assert(R.IsCurrentSpyGeneration(retry) and retry.spyID==spy.id,
            "retry barrier belongs to the archived spy");
        if R.Third then R.Third.OperationBarrier(retry,params.Action,spy.id); end
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
    local spy=assert(R.FindSpy(owner,params.NewID), "arrived native spy missing");
    assert(spy:GetOwner()==owner and spy:GetID()==params.NewID and
        spy:GetType()==GameInfo.Units["UNIT_SPY"].Index, "arrived native spy identity mismatch");
    local city=Cities.GetPlotPurchaseCity(Map.GetPlot(spy:GetX(),spy:GetY()));
    assert(city and city:GetOwner()==params.TargetOwner and city:GetID()==params.TargetID,
        "arrived native spy destination mismatch");
    local retry=Game:GetProperty(RETRY);
    if retry then
        assert(R.IsCurrentSpyGeneration(retry) and retry.spyID==params.OldID and
            (not R.Third or not R.Third.IsThird(retry) or R.Third.AllowedID(params.NewID)),
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
    local name=R.DecodeText(params.Name);
    local spy=assert(Game:GetProperty("JAC_BOUNDARY_SPY"),"native spy reference missing");
    assert(spy.owner==owner and type(params)=="table" and params.OldID==spy.id and
        type(name)=="string" and type(params.Level)=="number" and
        type(params.TargetOwner)=="number" and type(params.TargetID)=="number" and
        not Game:GetProperty("JAC_BOUNDARY_SPY_TRAVEL"),"invalid or repeated travel receipt");
    local city=assert(CityManager.GetCity(params.TargetOwner,params.TargetID),"travel target missing");
    assert(city:GetOwner()~=owner,"travel target is not foreign");
    Game:SetProperty("JAC_BOUNDARY_SPY_TRAVEL",{oldID=spy.id,nameText=R.EncodeText(name),level=params.Level,
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

local function OfficialCivicTitleDelta(civicType)
    local total=0;
    for link in GameInfo.CivicModifiers() do
        if link.CivicType==civicType then
            local modifier=assert(GameInfo.Modifiers[link.ModifierId],
                "native Civic title modifier missing");
            local definition=assert(GameInfo.DynamicModifiers[modifier.ModifierType],
                "native Civic title effect definition missing");
            if definition.EffectType=="EFFECT_ADJUST_PLAYER_GOVERNOR_POINTS" then
                local found=false;
                for argument in GameInfo.ModifierArguments() do
                    if argument.ModifierId==link.ModifierId and argument.Name=="Delta" then
                        local delta=tonumber(argument.Value);
                        assert(delta and delta>=0 and delta==math.floor(delta),
                            "official Civic Governor title delta is invalid");
                        total=total+delta; found=true;
                    end
                end
                assert(found,"official Civic Governor title delta missing");
            end
        end
    end
    return total;
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
        params.Action=="BEGIN_SPY_RETRY" or params.Action=="FINALIZE_SPY_RETRY" or
        params.Action=="PROGRESS_CAPACITY" or
        params.Action=="MARK_CAPTIVE_EXCHANGE" or
        params.Action=="PREPARE_DISTRICT_CIVIC" or
        params.Action=="RESUME_CAPACITY_CIVIC_READ" or
        params.Action=="PREPARE_TIER2_CAPACITY_CIVIC" or
        params.Action=="RESUME_TIER2_CAPACITY_CIVIC_READ" or
        params.Action=="PREPARE_TIER1_GOV_WINDOW_CIVIC" or
        params.Action=="CHANGE_CAPACITY_GOVERNMENT" or
        params.Action=="CLASSIFY_CAPACITY_GOVERNMENT" or
        params.Action=="READ_CAPACITY_GOVERNMENT_CLASSIFICATION" or
        params.Action=="PROBE_CAPACITY_GOVERNMENT_API" or
        params.Action=="PROGRESS_CAPACITY_DISTRICT" or
        params.Action=="THIRD_SPY_CIVIC_INTENT",
        "unknown enemy fixture barrier");
    local previous=Game:GetProperty("JAC_BOUNDARY_ENEMY_FIXTURE_BARRIER");
    local marker={action=params.Action,token=params.Token,turn=params.Turn,
        attacker=controller,phase=params.Phase,
        serial=(previous and previous.serial or 0)+1};
    Game:SetProperty("JAC_BOUNDARY_ENEMY_FIXTURE_BARRIER",marker);
    if params.Action=="PREPARE_DISTRICT_CIVIC" or
        params.Action=="RESUME_CAPACITY_CIVIC_READ" or
        params.Action=="PREPARE_TIER2_CAPACITY_CIVIC" or
        params.Action=="RESUME_TIER2_CAPACITY_CIVIC_READ" or
        params.Action=="PREPARE_TIER1_GOV_WINDOW_CIVIC" or
        params.Action=="CHANGE_CAPACITY_GOVERNMENT" then
        print("JAC_BOUNDARY CORE_CAPACITY_CIVIC_BARRIER "..R.Serialize(marker));
    end
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
        assert(R.IsCurrentSpyGeneration(retry) and retry.spyID and not R.IsArchivedSpyEvent(retry,params.MissionID),
            "archived failed native mission cannot be recorded for the retry");
        if R.Third then R.Third.RecordMission(controller,params); end
    end
    assert(c.cityID and type(params)=="table" and params.Attacker==c.attacker and
        type(params.MissionID)=="number" and params.MissionID>=0 and
        not Game:GetProperty("JAC_BOUNDARY_ENEMY_MISSION"), "invalid or repeated enemy mission event");
    Game:SetProperty("JAC_BOUNDARY_ENEMY_MISSION",{attacker=params.Attacker,
        id=params.MissionID,turn=Game.GetCurrentGameTurn()});
end);

-- Archive the verified failed attempt in test-only save data before replacing
-- any fixture pointers. Civ6's unit, mission history, Governor and counters are untouched.
local CAPACITY_RETRY_FIELDS={"Mode","Outcome","SpyID","CurrentID","EventID",
    "EventTurn","Operation","Plot","Initial","Escape","CompletionTurn",
    "Spent","Total","GovernorType","Capacity","Occupied","CaptiveCaptor",
    "CaptiveName","CaptiveNameIndex","CaptiveLevel","BindingType",
    "BindingCityID","BindingIndex","BindingToken","BindingTurn",
    "MapX","MapY","MapLevel","MapName","MapCount","CapturedCount",
    "OffMapCount","ActiveOffMapCount","PlaceholderIndex"};
local function HandleEnemySpyRetry(controller,params,final)
    EnemyReceipt(controller,params,final and "FINALIZE_SPY_RETRY" or "BEGIN_SPY_RETRY");
    local captiveName=R.DecodeText(params.CaptiveName);
    local returnName=R.DecodeText(params.ReturnName);
    local mapName=R.DecodeText(params.MapName);
    local pending=Game:GetProperty(RETRY_PENDING);
    local barrier=Game:GetProperty("JAC_BOUNDARY_ENEMY_FIXTURE_BARRIER");
    if final then
        assert(pending and params.Mode=="CAPACITY_MAP" and
            params.OriginalToken==pending.token and params.Token>pending.token and
            params.Turn==pending.turn and params.Frame>pending.frame and
            params.PublishEpoch>pending.publishEpoch and barrier and
            barrier.action=="BEGIN_SPY_RETRY" and barrier.token==pending.token and
            barrier.turn==pending.turn and barrier.attacker==controller,
            "capacity retry attestation token, turn or publish changed");
        for _,field in ipairs(CAPACITY_RETRY_FIELDS) do
            assert(params[field]==pending.observed[field],
                "capacity retry UI observation changed: "..field);
        end
    else
        assert(not pending,"capacity retry attestation already pending");
        assert(not barrier or params.Token>barrier.token,
            "enemy retry token is older than the saved sync barrier");
    end
    local c=EnemyConfig(controller);
    assert(c.cityID and type(params)=="table" and
        params.Action==(final and "FINALIZE_SPY_RETRY" or "BEGIN_SPY_RETRY") and
        params.Mode and (params.Mode=="PRODUCE" or params.Mode=="RETURN" or
        params.Mode=="MAP" or params.Mode=="CAPACITY_MAP") and
        not Game:GetProperty(RETRY),
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
    local mapCount=R.MapSpyCount(controller);
    local attackerDiplomacy=assert(Players[controller]:GetDiplomacy(),
        "native attacker diplomacy missing");
    local rosterCaptorID=c.owner;
    if params.Mode=="PRODUCE" and params.Outcome=="CAPTURED" then
        rosterCaptorID=params.CaptiveCaptor;
    end
    local rosterCaptor=assert(Players[rosterCaptorID],
        "native retry captor missing");
    local captorDiplomacy=assert(rosterCaptor:GetDiplomacy(),
        "native captor diplomacy missing");
    -- Classify each roster interface before observing or archiving anything.
    -- Historical routes support a wholly absent pair or a complete pair;
    -- partial pairs never establish the identity of an operative.
    local function PairedRosterAPI(diplomacy,countName,recordName,label)
        local count=type(diplomacy[countName])=="function";
        local record=type(diplomacy[recordName])=="function";
        if not count and not record then return false; end
        assert(count,"native Core retry "..label.." count API unavailable");
        assert(record,"native "..params.Mode.." "..label.." record API unavailable");
        return true;
    end
    local offMapAPI=PairedRosterAPI(attackerDiplomacy,
        "GetNumSpiesOffMap","GetNthOffMapSpy","off-map");
    local captiveAPI=PairedRosterAPI(captorDiplomacy,
        "GetNumSpiesCaptured","GetNthCapturedSpy","captive");
    local offMapCount,captiveCount,coreCapacity;
    if offMapAPI then
        offMapCount=attackerDiplomacy:GetNumSpiesOffMap();
    end
    if captiveAPI then
        captiveCount=captorDiplomacy:GetNumSpiesCaptured();
    end
    assert((not offMapAPI or (type(offMapCount)=="number" and offMapCount>=0)) and
        (not captiveAPI or (type(captiveCount)=="number" and captiveCount>=0)),
        "native Core retry roster count unavailable");
    if params.Mode=="PRODUCE" then
        assert(mapCount==0,
            "production retry requires a complete roster without a native map spy");
        if captiveCount~=nil then
            assert(captiveCount==(params.Outcome=="CAPTURED" and 1 or 0),
                "native PRODUCE captive roster changed");
            if captiveCount==1 then
                assert(type(captorDiplomacy.GetNthCapturedSpy)=="function",
                    "native PRODUCE captive record API unavailable");
                local held=assert(captorDiplomacy:GetNthCapturedSpy(rosterCaptorID,0),
                    "native PRODUCE captive record missing");
                assert(held.OwningPlayer==controller and
                    held.Name==captiveName and
                    held.NameIndex==params.CaptiveNameIndex and
                    held.Level==params.CaptiveLevel,
                    "native PRODUCE captive roster changed");
            end
        end
        if offMapCount~=nil then
            assert((params.Outcome=="CAPTURED" and offMapCount<=1) or
                (params.Outcome=="GONE_NO_CAPTURE_RECORD" and offMapCount==0),
                "native PRODUCE off-map roster changed");
            if offMapCount==1 then
                assert(type(attackerDiplomacy.GetNthOffMapSpy)=="function",
                    "native PRODUCE off-map record API unavailable");
                local placeholder=assert(attackerDiplomacy:GetNthOffMapSpy(controller,0),
                    "native PRODUCE captive placeholder missing");
                local defaultName=GameInfo.Units["UNIT_SPY"].Name;
                local sameName=placeholder.Name==captiveName or
                    (placeholder.Name=="" and captiveName==defaultName) or
                    (captiveName=="" and placeholder.Name==defaultName);
                assert(sameName and placeholder.Level==params.CaptiveLevel,
                    "native PRODUCE captive placeholder identity changed");
                assert(placeholder.ReturnTurn==-1 and
                    placeholder.XLocation==-9999 and placeholder.YLocation==-9999,
                    "native PRODUCE off-map roster changed");
            end
        end
    elseif params.Mode=="CAPACITY_MAP" then
        local building=assert(GameInfo.Buildings["BUILDING_GOV_SPIES"],
            "native Intelligence Agency definition missing");
        local progress=assert(Game:GetProperty("JAC_BOUNDARY_CAPACITY_PROGRESS"),
            "native Intelligence Agency progress receipt missing");
        local latest=assert(progress.latest,"latest native capacity receipt missing");
        local step=progress.steps and progress.steps.BUILDING_GOV_SPIES;
        local city=assert(CityManager.GetCity(controller,params.BindingCityID),
            "native Intelligence Agency production city missing");
        assert(params.Outcome=="CAPTURED_WITH_NEW_MAP" and
            params.Capacity==2 and params.Occupied==2 and
            params.CurrentID~=spy.id and
            params.BindingType==building.BuildingType and
            params.BindingIndex==building.Index and
            step and latest.buildingType==building.BuildingType and
            latest.buildingIndex==building.Index and
            latest.cityID==params.BindingCityID and
            latest.token==params.BindingToken and
            latest.turn==params.BindingTurn and
            step.cityID==latest.cityID and step.token==latest.token and
            step.turn==latest.turn and step.buildingIndex==latest.buildingIndex and
            latest.turn<=Game.GetCurrentGameTurn() and
            city:GetOwner()==controller and
            city:GetBuildings():HasBuilding(building.Index),
            "new spy is not tied to the completed native Intelligence Agency");
        local capacityGetterType=type(attackerDiplomacy.GetSpyCapacity);
        print("JAC_BOUNDARY CORE_CAPACITY_RETRY_API_TYPES "..R.Serialize({
            capacityGetter=capacityGetterType,
            offMapCount=type(attackerDiplomacy.GetNumSpiesOffMap),
            offMapRecord=type(attackerDiplomacy.GetNthOffMapSpy),
            captiveCount=type(captorDiplomacy.GetNumSpiesCaptured),
            captiveRecord=type(captorDiplomacy.GetNthCapturedSpy)}));
        if capacityGetterType=="function" then
            coreCapacity=attackerDiplomacy:GetSpyCapacity();
            assert(coreCapacity==params.Capacity,
                "native Core spy capacity changed after UI observation");
        end
        print("JAC_BOUNDARY CORE_CAPACITY_RETRY_IDENTITY "..R.Serialize({
            mapCount=mapCount,captor=params.CaptiveCaptor,expectedCaptor=c.owner,
            map=params.MapCount,captured=params.CapturedCount,offMap=params.OffMapCount,
            moving=params.ActiveOffMapCount,placeholder=params.PlaceholderIndex,
            captiveName=captiveName,captiveNameIndex=params.CaptiveNameIndex,
            captiveLevel=params.CaptiveLevel,travelName=R.StoredText(travel,"name"),travelLevel=travel.level,
            defaultName=GameInfo.Units["UNIT_SPY"].Name,
            mapX=params.MapX,mapY=params.MapY,mapLevel=params.MapLevel,mapName=mapName}));
        assert(params.CaptiveCaptor==c.owner and mapCount==1 and
            params.MapCount==1 and params.CapturedCount==1 and
            params.OffMapCount==1 and params.ActiveOffMapCount==0 and
            params.PlaceholderIndex==1 and
            params.CaptiveLevel==travel.level and
            (captiveName==R.StoredText(travel,"name") or
             (captiveName=="" and params.CaptiveNameIndex==-1 and
              R.StoredText(travel,"name")==GameInfo.Units["UNIT_SPY"].Name)) and
            type(params.MapX)=="number" and type(params.MapY)=="number" and
            type(params.MapLevel)=="number" and type(mapName)=="string" and
            params.MapLevel>=0,
            "capacity retry needs exactly one native map spy and one old captive");
        local captive;
        if captiveAPI then
            assert(captiveCount==1,"native captive roster changed between UI and Gameplay");
            captive=assert(captorDiplomacy:GetNthCapturedSpy(c.owner,0),
                "original native captive unavailable in Gameplay");
            assert(captive.OwningPlayer==controller and
                captive.Name==captiveName and
                captive.NameIndex==params.CaptiveNameIndex and
                captive.Level==params.CaptiveLevel and captive.Level==travel.level,
                "original native captive differs in Gameplay");
        end
        if offMapAPI then
            assert(offMapCount==1,
                "native off-map roster changed between UI and Gameplay");
            local placeholder=assert(attackerDiplomacy:GetNthOffMapSpy(controller,0),
                "original captive placeholder unavailable in Gameplay");
            local defaultName=GameInfo.Units["UNIT_SPY"].Name;
            local sameName=placeholder.Name==captiveName or
                (placeholder.Name=="" and captiveName==defaultName) or
                (captiveName=="" and placeholder.Name==defaultName);
            assert(sameName and placeholder.Level==params.CaptiveLevel and
                placeholder.ReturnTurn==-1 and placeholder.XLocation==-9999 and
                placeholder.YLocation==-9999,
                "off-map roster contains a moving spy or a changed captive placeholder");
        end
        local current=assert(R.FindSpy(controller,params.CurrentID),
            "new native map spy exact ID missing in Gameplay");
        assert(current:GetID()==params.CurrentID and current:GetOwner()==controller and
            current:GetType()==GameInfo.Units["UNIT_SPY"].Index and
            current:GetX()==params.MapX and current:GetY()==params.MapY and
            Map.GetPlot(current:GetX(),current:GetY()),
            "new native map spy type, owner or plot changed in Gameplay");
    elseif params.Mode=="MAP" then
        assert(mapCount==1 and (offMapCount==nil or offMapCount==0) and
            (captiveCount==nil or captiveCount==0),
            "native MAP roster changed between UI and Gameplay");
        local current=assert(R.FindSpy(controller,params.CurrentID),
            "living native map spy missing at archive");
        assert(current:GetOwner()==controller and
            current:GetType()==GameInfo.Units["UNIT_SPY"].Index,
            "living native map spy identity differs at archive");
    elseif params.Mode=="RETURN" then
        assert(mapCount==0 and (offMapCount==nil or offMapCount==1) and
            (captiveCount==nil or captiveCount==0),
            "native RETURN roster changed between UI and Gameplay");
        if offMapCount==1 then
            assert(type(attackerDiplomacy.GetNthOffMapSpy)=="function",
                "native RETURN off-map record API unavailable");
            local returning=assert(attackerDiplomacy:GetNthOffMapSpy(controller,0),
                "native RETURN off-map record missing");
            assert(returning.Name==returnName and
                returning.Level==params.ReturnLevel and
                returning.ReturnTurn==params.ReturnTurn and
                returning.XLocation==params.ReturnX and
                returning.YLocation==params.ReturnY,
                "native RETURN roster changed between UI and Gameplay");
        end
    end
    local coreRoster={map=mapCount,offMap=offMapCount,captured=captiveCount,
        mapSource="Core",offMapAPI=offMapAPI,captiveAPI=captiveAPI,
        offMapSource=offMapAPI and "Core" or "UI",
        capturedSource=captiveAPI and "Core" or "UI"};
    if params.Mode=="CAPACITY_MAP" then
        coreRoster.capacitySource=type(attackerDiplomacy.GetSpyCapacity)=="function" and
            "Core" or "UI";
        coreRoster.capacityGetterType=type(attackerDiplomacy.GetSpyCapacity);
        coreRoster.capacity=coreCapacity;
        if final then
            assert(pending.coreRoster.offMapAPI==offMapAPI and
                pending.coreRoster.captiveAPI==captiveAPI and
                pending.coreRoster.capacityGetterType==coreRoster.capacityGetterType,
                "capacity retry Core API availability changed");
        else
            local observed={};
            for _,field in ipairs(CAPACITY_RETRY_FIELDS) do
                observed[field]=params[field];
            end
            Game:SetProperty(RETRY_PENDING,{token=params.Token,turn=params.Turn,
                frame=params.Frame,publishEpoch=params.PublishEpoch,
                observed=observed,coreRoster=coreRoster});
            print("JAC_BOUNDARY ENEMY_SPY_RETRY_ATTESTED "..
                R.Serialize(Game:GetProperty(RETRY_PENDING)));
            return;
        end
    end
    local archive={spyID=spy.id,eventID=event.id,eventTurn=event.turn,
        operation=params.Operation,plot=params.Plot,initial=params.Initial,
        escape=params.Escape,completionTurn=params.CompletionTurn,
        travelOldID=travel.oldID,travelNameText=R.EncodeText(R.StoredText(travel,"name")),travelLevel=travel.level,
        targetOwner=travel.targetOwner,targetID=travel.targetID,
        governorType=params.GovernorType,governorNeutralized=native.neutralized,
        governorCityID=native.cityID,spent=params.Spent,total=params.Total,
        merit=R.Property(c.leader,"MERIT") or 0,rosterOutcome=params.Outcome,
        captiveCaptor=params.CaptiveCaptor,captiveNameText=params.CaptiveName,
        captiveNameIndex=params.CaptiveNameIndex,captiveLevel=params.CaptiveLevel,
        capacity=params.Capacity,occupied=params.Occupied,archivedTurn=Game.GetCurrentGameTurn()};
    local retry={current=2,mode=params.Mode,archive=archive,
        spyID=spy.id,
        coreRoster=coreRoster,
        returnNameText=params.ReturnName,returnLevel=params.ReturnLevel,
        returnTurn=params.ReturnTurn,returnX=params.ReturnX,returnY=params.ReturnY};
    if final then
        retry.attestation={initialToken=pending.token,finalToken=params.Token,
            turn=params.Turn,capacitySource=coreRoster.capacitySource,
            capturedSource=coreRoster.capturedSource,
            offMapSource=coreRoster.offMapSource,
            initialCoreRoster=pending.coreRoster};
        retry.uiRoster={source="UI",map=params.MapCount,
            captured=params.CapturedCount,offMap=params.OffMapCount,
            activeOffMap=params.ActiveOffMapCount,
            placeholderIndex=params.PlaceholderIndex,
            capacity=params.Capacity,mapX=params.MapX,mapY=params.MapY,
            mapLevel=params.MapLevel,mapNameText=params.MapName};
    end
    if params.Mode=="PRODUCE" then retry.spyID=nil; end
    if params.Mode=="MAP" or params.Mode=="CAPACITY_MAP" then
        retry.spyID=params.CurrentID;
    end
    if params.Mode=="CAPACITY_MAP" then
        retry.binding={source="BUILDING_GOV_SPIES",spyID=params.CurrentID,
            cityID=params.BindingCityID,buildingIndex=params.BindingIndex,
            token=params.BindingToken,turn=params.BindingTurn};
    end
    Game:SetProperty(RETRY,retry);
    Game:SetProperty("JAC_BOUNDARY_ENEMY_MISSION",nil);
    Game:SetProperty("JAC_BOUNDARY_SPY_BARRIER",nil);
    Game:SetProperty("JAC_BOUNDARY_SPY_TRAVEL",nil);
    if params.Mode=="PRODUCE" then
        Game:SetProperty("JAC_BOUNDARY_SPY",nil);
        Game:SetProperty("JAC_BOUNDARY_SPY_PREPARED",nil);
        Game:SetProperty("JAC_BOUNDARY_SPY_PROGRESS",nil);
        Game:SetProperty(PRODUCTION,nil);
    elseif (params.Mode=="MAP" or params.Mode=="CAPACITY_MAP") and
        params.CurrentID~=spy.id then
        Game:SetProperty("JAC_BOUNDARY_SPY",{owner=controller,id=params.CurrentID});
    end
    if final then Game:SetProperty(RETRY_PENDING,nil); end
    print("JAC_BOUNDARY ENEMY_SPY_RETRY_ARCHIVED "..R.Serialize(retry));
end
GameEvents.JAC_BoundaryBeginEnemySpyRetry.Add(function(controller,params)
    HandleEnemySpyRetry(controller,params,false);
end);
GameEvents.JAC_BoundaryFinalizeEnemySpyRetry.Add(function(controller,params)
    HandleEnemySpyRetry(controller,params,true);
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
    -- This durable entry distinguishes a missing callback from a callback that
    -- reached SetCivic and returned before the native culture cache published.
    -- A saved or uncertain attempt is never sent to SetCivic a second time.
    if Game:GetProperty(CAPACITY_CIVIC_RECEIPT) then
        print("JAC_BOUNDARY CORE_CAPACITY_CIVIC_DUPLICATE_SUPPRESSED");
        return;
    end
    local receipt={action="PREPARE_DISTRICT_CIVIC",owner=controller,
        token=type(params)=="table" and params.Token or -1,
        turn=Game.GetCurrentGameTurn(),generation=1,phase="ENTERED",status="ENTERED"};
    Game:SetProperty(CAPACITY_CIVIC_RECEIPT,receipt);
    print("JAC_BOUNDARY CORE_CAPACITY_CIVIC_ENTRY "..R.Serialize(receipt));
    local ok,err=pcall(function()
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
        local total,spent=titles:GetGovernorPoints(),titles:GetGovernorPointsSpent();
        assert(params.CivicIndex==civic.Index and not culture:HasCivic(civic.Index) and
            total==params.Total and spent==params.Spent,
            "native capacity Civic or title accounting changed");
        assert(params.OriginalOwner==controller and
            params.OriginalIndex==civic.Index and
            params.OriginalToken==params.Token and
            params.OriginalTurn==params.Turn and
            params.OriginalSpent==spent and
            params.OriginalSequence==params.Token,
            "native capacity Civic original request identity differs");
        assert(type(culture.SetCivic)=="function","native civic research API missing");
        assert(not Game:GetProperty(CAPACITY_CIVIC_ATTEMPT),
            "native capacity Civic setter attempt was already recorded");
        local attempt={owner=controller,index=civic.Index,token=params.Token,
            sequence=params.Token,turn=Game.GetCurrentGameTurn(),
            beforeTotal=total,beforeSpent=spent,generation=receipt.generation};
        Game:SetProperty(CAPACITY_CIVIC_ATTEMPT,attempt);
        local saved=assert(Game:GetProperty(CAPACITY_CIVIC_ATTEMPT),
            "native capacity Civic setter attempt did not persist");
        assert(saved.owner==attempt.owner and saved.index==attempt.index and
            saved.token==attempt.token and saved.sequence==attempt.sequence and
            saved.turn==attempt.turn and saved.beforeSpent==attempt.beforeSpent,
            "native capacity Civic setter attempt changed before mutation");
        print("JAC_BOUNDARY CORE_CAPACITY_CIVIC_ATTEMPT "..R.Serialize(attempt));
        receipt.index=civic.Index;
        receipt.beforeTotal=total; receipt.beforeSpent=spent;
        receipt.phase="VALIDATED";
        Game:SetProperty(CAPACITY_CIVIC_RECEIPT,receipt);
        receipt.phase="SETTER_ENTERED";
        Game:SetProperty(CAPACITY_CIVIC_RECEIPT,receipt);
        print("JAC_BOUNDARY CORE_CAPACITY_CIVIC_SETTER_ENTRY "..R.Serialize(receipt));
        culture:SetCivic(civic.Index,true);
        receipt.phase="SETTER_RETURNED";
        receipt.afterTotal=titles:GetGovernorPoints();
        receipt.afterSpent=titles:GetGovernorPointsSpent();
        receipt.owned=culture:HasCivic(civic.Index);
        receipt.status=receipt.owned and "OBSERVED" or "PENDING_NATIVE";
        Game:SetProperty(CAPACITY_CIVIC_RECEIPT,receipt);
    end);
    if not ok then
        receipt.status="ERROR"; receipt.error="CALLBACK_FAILED";
        Game:SetProperty(CAPACITY_CIVIC_RECEIPT,receipt);
        print("JAC_BOUNDARY CORE_CAPACITY_CIVIC_ERROR "..tostring(err));
    end
    print("JAC_BOUNDARY CORE_CAPACITY_CIVIC_RESULT "..R.Serialize(receipt));
end);

-- A separate, ordered Gameplay read proves what the engine exposes after the
-- command barrier. It does not repeat the setter or manufacture Civic ownership.
GameEvents.JAC_BoundaryReadCapacityCivic.Add(function(controller,params)
    local previous=Game:GetProperty(CAPACITY_CIVIC_READ);
    if previous and previous.action=="RESUME_CAPACITY_CIVIC_READ" then
        print("JAC_BOUNDARY CORE_CAPACITY_CIVIC_READ_DUPLICATE_SUPPRESSED");
        return;
    end
    local receipt=Game:GetProperty(CAPACITY_CIVIC_RECEIPT);
    local attempt=Game:GetProperty(CAPACITY_CIVIC_ATTEMPT);
    local read={owner=controller,token=type(params)=="table" and params.Token or -1,
        action=type(params)=="table" and params.Action or "",
        generation=attempt and attempt.generation or -1,
        originalToken=attempt and attempt.token or -1,
        originalTurn=attempt and attempt.turn or -1,
        serial=(previous and previous.serial or 0)+1,
        turn=Game.GetCurrentGameTurn(),
        frame=type(params)=="table" and params.Frame or -1,
        publishEpoch=type(params)=="table" and params.PublishEpoch or -1,
        status="ENTERED"};
    Game:SetProperty(CAPACITY_CIVIC_READ,read);
    print("JAC_BOUNDARY CORE_CAPACITY_CIVIC_READ_ENTRY "..R.Serialize(read));
    local ok,err=pcall(function()
        assert(params.Action=="PREPARE_DISTRICT_CIVIC" or
            params.Action=="RESUME_CAPACITY_CIVIC_READ",
            "capacity Civic read action differs");
        EnemyReceipt(controller,params,params.Action);
        assert((params.Action=="PREPARE_DISTRICT_CIVIC" and params.Phase=="READ") or
            (params.Action=="RESUME_CAPACITY_CIVIC_READ" and
             params.Phase=="RESUME_READ"), "capacity Civic read phase differs");
        assert(attempt and receipt and receipt.phase=="SETTER_RETURNED" and
            receipt.status~="ERROR" and receipt.owner==attempt.owner and
            receipt.index==attempt.index and receipt.token==attempt.token and
            receipt.turn==attempt.turn and receipt.beforeTotal==attempt.beforeTotal and
            receipt.beforeSpent==attempt.beforeSpent and
            receipt.generation==attempt.generation,
            "capacity Civic setter receipt missing or changed");
        assert(controller==attempt.owner and
            params.OriginalOwner==attempt.owner and
            params.OriginalIndex==attempt.index and
            params.OriginalToken==attempt.token and
            params.OriginalTurn==attempt.turn and
            params.OriginalSpent==attempt.beforeSpent and
            params.OriginalSequence==attempt.sequence,
            "capacity Civic read does not reference the original setter attempt");
        assert((params.Action=="PREPARE_DISTRICT_CIVIC" and
            params.Token==attempt.token and params.Turn==attempt.turn) or
            (params.Action=="RESUME_CAPACITY_CIVIC_READ" and
             params.Token>attempt.token and params.Turn==attempt.turn+1),
            "capacity Civic read is not in its allowed turn");
        local civic=assert(GameInfo.Civics["CIVIC_JAC_SENGOKU_DOMAIN_LAW"],
            "native Government Plaza Civic missing at read");
        assert(attempt.index==civic.Index,"capacity Civic command index changed");
        local player=assert(Players[controller],"capacity Civic read owner missing");
        local titles=player:GetGovernors();
        read.index=civic.Index;
        read.owned=player:GetCulture():HasCivic(civic.Index);
        read.total=titles:GetGovernorPoints();
        read.spent=titles:GetGovernorPointsSpent();
        read.status=read.owned and "OBSERVED" or "PENDING_NATIVE";
        if read.owned and read.spent==attempt.beforeSpent then
            Game:SetProperty("JAC_BOUNDARY_CAPACITY_CIVIC",{
                owner=controller,index=civic.Index,token=params.Token,
                generation=attempt.generation,readSerial=read.serial,
                originalToken=attempt.token,originalSequence=attempt.sequence,
                readFrame=read.frame,readPublishEpoch=read.publishEpoch,
                beforeTotal=attempt.beforeTotal,beforeSpent=attempt.beforeSpent,
                afterTotal=read.total,afterSpent=read.spent,
                commandTurn=attempt.turn,turn=read.turn});
        end
        Game:SetProperty(CAPACITY_CIVIC_READ,read);
    end);
    if not ok then
        read.status="ERROR"; read.error="READ_FAILED";
        Game:SetProperty(CAPACITY_CIVIC_READ,read);
        print("JAC_BOUNDARY CORE_CAPACITY_CIVIC_READ_ERROR "..tostring(err));
    end
    print("JAC_BOUNDARY CORE_CAPACITY_CIVIC_READ_RESULT "..R.Serialize(read));
end);

-- One additional, scoped official Civic research can unlock a retained Tier 2
-- Government. It has its own immutable attempt; the Domain Law receipt and
-- completed native district remain untouched.
GameEvents.JAC_BoundaryPrepareTier2CapacityCivic.Add(function(controller,params)
    if Game:GetProperty(TIER2_CIVIC_RECEIPT) or Game:GetProperty(TIER2_CIVIC_ATTEMPT) then
        print("JAC_BOUNDARY CORE_TIER2_CIVIC_DUPLICATE_SUPPRESSED");
        return;
    end
    local receipt={action="PREPARE_TIER2_CAPACITY_CIVIC",owner=controller,
        token=type(params)=="table" and params.Token or -1,
        turn=Game.GetCurrentGameTurn(),generation=1,phase="ENTERED",status="ENTERED"};
    Game:SetProperty(TIER2_CIVIC_RECEIPT,receipt);
    print("JAC_BOUNDARY CORE_TIER2_CIVIC_ENTRY "..R.Serialize(receipt));
    local ok,err=pcall(function()
        EnemyReceipt(controller,params,"PREPARE_TIER2_CAPACITY_CIVIC");
        EnemyConfig(controller);
        assert(params.TargetGovernment=="GOVERNMENT_MERCHANT_REPUBLIC" or
            params.TargetGovernment=="GOVERNMENT_MONARCHY",
            "Tier 2 capacity Government is not retained by the ruleset");
        local government=assert(GameInfo.Governments[params.TargetGovernment],
            "retained Tier 2 Government missing");
        assert(government.Tier=="Tier2" and government.PrereqCivic==params.CivicType,
            "Tier 2 Government Civic prerequisite changed");
        local civic=assert(GameInfo.Civics[params.CivicType],
            "Tier 2 Government prerequisite Civic missing");
        local player=assert(Players[controller],"Tier 2 Civic owner missing");
        local culture=player:GetCulture();
        local prerequisiteCount=0;
        for link in GameInfo.CivicPrereqs() do
            if link.Civic==params.CivicType then
                local prerequisite=assert(GameInfo.Civics[link.PrereqCivic],
                    "Tier 2 Government Civic prerequisite definition missing");
                assert(culture:HasCivic(prerequisite.Index),
                    "Tier 2 Government Civic prerequisite is unresearched");
                prerequisiteCount=prerequisiteCount+1;
            end
        end
        assert(prerequisiteCount>0,
            "Tier 2 Government Civic has no proven Sengoku prerequisite");
        local titles=player:GetGovernors();
        local nativeGov=assert(ExposedMembers.JAC_PersonaBoundaryGovernmentNativeRead,
            "UI native Government reader unavailable")(controller);
        assert(nativeGov.owner==controller and nativeGov.turn==params.Turn and
            nativeGov.frame>=params.Frame and
            nativeGov.publishEpoch>=params.PublishEpoch,
            "native Government reader did not observe this request");
        local current=GameInfo.Governments[nativeGov.index];
        assert(current and current.Tier=="Tier1",
            "Tier 2 Civic requires the completed Tier 1 Government path");
        local tier1Built=false;
        for _,city in player:GetCities():Members() do
            for _,kind in ipairs({"BUILDING_GOV_TALL","BUILDING_GOV_CONQUEST",
                "BUILDING_GOV_WIDE"}) do
                local row=GameInfo.Buildings[kind];
                if row and city:GetBuildings():HasBuilding(row.Index) then
                    tier1Built=true;
                end
            end
        end
        assert(tier1Built,"Tier 2 Civic requires an actual Tier 1 Plaza building");
        local total,spent=titles:GetGovernorPoints(),titles:GetGovernorPointsSpent();
        assert(params.CivicIndex==civic.Index and not culture:HasCivic(civic.Index) and
            params.Total==total and params.Spent==spent and
            params.OriginalOwner==controller and params.OriginalIndex==civic.Index and
            params.OriginalToken==params.Token and params.OriginalTurn==params.Turn and
            params.OriginalSpent==spent and params.OriginalSequence==params.Token,
            "Tier 2 Civic or original title accounting changed");
        assert(type(culture.SetCivic)=="function","native Civic setter unavailable");
        local expectedDelta=OfficialCivicTitleDelta(civic.CivicType);
        local attempt={owner=controller,index=civic.Index,civicType=civic.CivicType,
            targetGovernment=params.TargetGovernment,
            token=params.Token,sequence=params.Token,
            turn=Game.GetCurrentGameTurn(),beforeTotal=total,beforeSpent=spent,
            generation=receipt.generation,expectedTitleDelta=expectedDelta};
        Game:SetProperty(TIER2_CIVIC_ATTEMPT,attempt);
        local saved=assert(Game:GetProperty(TIER2_CIVIC_ATTEMPT),
            "Tier 2 Civic setter attempt did not persist");
        assert(saved.owner==attempt.owner and saved.index==attempt.index and
            saved.token==attempt.token and saved.beforeSpent==attempt.beforeSpent,
            "Tier 2 Civic setter attempt changed before mutation");
        receipt.index=civic.Index; receipt.civicType=civic.CivicType;
        receipt.targetGovernment=params.TargetGovernment;
        receipt.beforeTotal=total; receipt.beforeSpent=spent;
        receipt.expectedTitleDelta=expectedDelta;
        receipt.phase="SETTER_ENTERED";
        Game:SetProperty(TIER2_CIVIC_RECEIPT,receipt);
        print("JAC_BOUNDARY CORE_TIER2_CIVIC_SETTER_ENTRY "..R.Serialize(attempt));
        culture:SetCivic(civic.Index,true);
        receipt.phase="SETTER_RETURNED";
        receipt.afterTotal=titles:GetGovernorPoints();
        receipt.afterSpent=titles:GetGovernorPointsSpent();
        receipt.owned=culture:HasCivic(civic.Index);
        receipt.status=receipt.owned and
            receipt.afterTotal==total+expectedDelta and
            receipt.afterSpent==spent and "OBSERVED" or "PENDING_NATIVE";
        Game:SetProperty(TIER2_CIVIC_RECEIPT,receipt);
    end);
    if not ok then
        receipt.status="ERROR"; receipt.error="CALLBACK_FAILED";
        Game:SetProperty(TIER2_CIVIC_RECEIPT,receipt);
        print("JAC_BOUNDARY CORE_TIER2_CIVIC_ERROR "..tostring(err));
    end
    print("JAC_BOUNDARY CORE_TIER2_CIVIC_RESULT "..R.Serialize(receipt));
end);

GameEvents.JAC_BoundaryReadTier2CapacityCivic.Add(function(controller,params)
    local prior=Game:GetProperty(TIER2_CIVIC_READ);
    if prior and prior.action=="RESUME_TIER2_CAPACITY_CIVIC_READ" then
        print("JAC_BOUNDARY CORE_TIER2_CIVIC_READ_DUPLICATE_SUPPRESSED");
        return;
    end
    local attempt=Game:GetProperty(TIER2_CIVIC_ATTEMPT);
    local receipt=Game:GetProperty(TIER2_CIVIC_RECEIPT);
    local read={owner=controller,token=type(params)=="table" and params.Token or -1,
        action=type(params)=="table" and params.Action or "",
        generation=attempt and attempt.generation or -1,
        originalToken=attempt and attempt.token or -1,
        originalTurn=attempt and attempt.turn or -1,
        serial=(prior and prior.serial or 0)+1,turn=Game.GetCurrentGameTurn(),
        frame=type(params)=="table" and params.Frame or -1,
        publishEpoch=type(params)=="table" and params.PublishEpoch or -1,
        status="ENTERED"};
    Game:SetProperty(TIER2_CIVIC_READ,read);
    local ok,err=pcall(function()
        assert(params.Action=="PREPARE_TIER2_CAPACITY_CIVIC" or
            params.Action=="RESUME_TIER2_CAPACITY_CIVIC_READ",
            "unknown Tier 2 Civic read action");
        EnemyReceipt(controller,params,params.Action);
        assert((params.Action=="PREPARE_TIER2_CAPACITY_CIVIC" and
            params.Phase=="READ") or
            (params.Action=="RESUME_TIER2_CAPACITY_CIVIC_READ" and
             params.Phase=="RESUME_READ"), "Tier 2 Civic read phase changed");
        assert(attempt and receipt and receipt.phase=="SETTER_RETURNED" and
            receipt.status~="ERROR" and receipt.owner==attempt.owner and
            receipt.index==attempt.index and receipt.token==attempt.token and
            receipt.turn==attempt.turn and receipt.beforeSpent==attempt.beforeSpent and
            receipt.generation==attempt.generation,
            "Tier 2 Civic setter attempt or result missing");
        assert(controller==attempt.owner and
            params.OriginalOwner==attempt.owner and
            params.OriginalIndex==attempt.index and
            params.OriginalToken==attempt.token and
            params.OriginalTurn==attempt.turn and
            params.OriginalSpent==attempt.beforeSpent and
            params.OriginalSequence==attempt.sequence and
            params.TargetGovernment==attempt.targetGovernment,
            "Tier 2 Civic read does not reference the original setter");
        assert((params.Action=="PREPARE_TIER2_CAPACITY_CIVIC" and
            params.Token==attempt.token and params.Turn==attempt.turn) or
            (params.Action=="RESUME_TIER2_CAPACITY_CIVIC_READ" and
             params.Token>attempt.token and params.Turn==attempt.turn+1),
            "Tier 2 Civic read uses the wrong native turn");
        local civic=assert(GameInfo.Civics[attempt.civicType],
            "Tier 2 prerequisite Civic definition disappeared");
        assert(civic.Index==attempt.index,"Tier 2 Civic index changed");
        local player=assert(Players[controller],"Tier 2 Civic read owner missing");
        read.index=civic.Index;
        read.owned=player:GetCulture():HasCivic(civic.Index);
        read.total=player:GetGovernors():GetGovernorPoints();
        read.spent=player:GetGovernors():GetGovernorPointsSpent();
        read.status=read.owned and "OBSERVED" or "PENDING_NATIVE";
        if read.owned and read.spent==attempt.beforeSpent and
            read.total==attempt.beforeTotal+attempt.expectedTitleDelta then
            Game:SetProperty(TIER2_CIVIC_MARKER,{
                owner=controller,index=civic.Index,civicType=attempt.civicType,
                targetGovernment=attempt.targetGovernment,
                token=params.Token,generation=attempt.generation,
                readSerial=read.serial,originalToken=attempt.token,
                originalSequence=attempt.sequence,
                readFrame=read.frame,readPublishEpoch=read.publishEpoch,
                beforeTotal=attempt.beforeTotal,beforeSpent=attempt.beforeSpent,
                afterTotal=read.total,afterSpent=read.spent,
                commandTurn=attempt.turn,turn=read.turn});
        end
        Game:SetProperty(TIER2_CIVIC_READ,read);
    end);
    if not ok then
        read.status="ERROR"; read.error="READ_FAILED";
        Game:SetProperty(TIER2_CIVIC_READ,read);
        print("JAC_BOUNDARY CORE_TIER2_CIVIC_READ_ERROR "..tostring(err));
    end
    print("JAC_BOUNDARY CORE_TIER2_CIVIC_READ_RESULT "..R.Serialize(read));
end);

-- A saved Domain Law completion is one turn too old to open the official
-- GovernmentScreen change gate. Research exactly one other eligible Sengoku
-- Civic, with the normal native title effect, on the Government-change turn.
GameEvents.JAC_BoundaryPrepareTier1GovernmentWindowCivic.Add(function(controller,params)
    if Game:GetProperty(GOV_WINDOW_ATTEMPT) or Game:GetProperty(GOV_WINDOW_RECEIPT) then
        print("JAC_BOUNDARY CORE_GOV_WINDOW_CIVIC_DUPLICATE_SUPPRESSED");
        return;
    end
    local receipt={action="PREPARE_TIER1_GOV_WINDOW_CIVIC",owner=controller,
        token=type(params)=="table" and params.Token or -1,
        turn=Game.GetCurrentGameTurn(),phase="ENTERED",status="ENTERED"};
    Game:SetProperty(GOV_WINDOW_RECEIPT,receipt);
    local ok,err=pcall(function()
        EnemyReceipt(controller,params,"PREPARE_TIER1_GOV_WINDOW_CIVIC");
        EnemyConfig(controller);
        local player=Players[controller];
        local culture=player:GetCulture();
        local government=assert(GameInfo.Governments["GOVERNMENT_OLIGARCHY"],
            "retained Tier 1 Government missing");
        local domain=assert(GameInfo.Civics[government.PrereqCivic],
            "Tier 1 Government prerequisite missing");
        local nativeGov=assert(ExposedMembers.JAC_PersonaBoundaryGovernmentNativeRead,
            "UI native Government reader unavailable")(controller);
        assert(government.Tier=="Tier1" and culture:HasCivic(domain.Index) and
            nativeGov.owner==controller and nativeGov.turn==params.Turn and
            nativeGov.index==params.BeforeGovernment and
            nativeGov.frame>=params.Frame and
            nativeGov.publishEpoch>=params.PublishEpoch and
            GameInfo.Governments[params.BeforeGovernment].Tier==nil,
            "Tier 1 Government or saved Domain Law differs");
        local allowed={CIVIC_JAC_SENGOKU_LEARNING_AND_RECORDS=true,
            CIVIC_JAC_SENGOKU_MILITARY_SERVICE_LEDGER=true,
            CIVIC_JAC_SENGOKU_PASSES_AND_PASSAGE=true};
        local civic=assert(GameInfo.Civics[params.CivicType],
            "selected Sengoku Government-window Civic missing");
        assert(allowed[params.CivicType] and civic.Index==params.CivicIndex and
            not culture:HasCivic(civic.Index),
            "Government-window Civic is already owned or outside the scoped candidates");
        local prerequisites=0;
        for link in GameInfo.CivicPrereqs() do
            if link.Civic==params.CivicType then
                local required=assert(GameInfo.Civics[link.PrereqCivic],
                    "Government-window prerequisite definition missing");
                assert(culture:HasCivic(required.Index),
                    "Government-window Civic has an unresearched prerequisite");
                prerequisites=prerequisites+1;
            end
        end
        assert(prerequisites>0,"Government-window Civic has no proven Sengoku prerequisite");
        local titles=player:GetGovernors();
        local total,spent=titles:GetGovernorPoints(),titles:GetGovernorPointsSpent();
        assert(params.Total==total and params.Spent==spent and
            params.OriginalOwner==controller and params.OriginalIndex==civic.Index and
            params.OriginalToken==params.Token and params.OriginalTurn==params.Turn and
            params.OriginalSpent==spent and params.OriginalSequence==params.Token,
            "Government-window Civic or official title baseline changed");
        assert(type(culture.SetCivic)=="function","native Civic setter unavailable");
        local expected=OfficialCivicTitleDelta(params.CivicType);
        local attempt={owner=controller,index=civic.Index,civicType=params.CivicType,
            targetGovernment=government.GovernmentType,
            token=params.Token,sequence=params.Token,turn=params.Turn,
            beforeGovernment=params.BeforeGovernment,
            beforeTotal=total,beforeSpent=spent,expectedTitleDelta=expected};
        Game:SetProperty(GOV_WINDOW_ATTEMPT,attempt);
        local saved=assert(Game:GetProperty(GOV_WINDOW_ATTEMPT),
            "Government-window Civic attempt did not persist");
        assert(saved.owner==attempt.owner and saved.index==attempt.index and
            saved.token==attempt.token and saved.beforeTotal==attempt.beforeTotal and
            saved.beforeSpent==attempt.beforeSpent,
            "Government-window Civic attempt changed before mutation");
        receipt.index=civic.Index; receipt.civicType=params.CivicType;
        receipt.beforeTotal=total; receipt.beforeSpent=spent;
        receipt.expectedTitleDelta=expected; receipt.phase="SETTER_ENTERED";
        Game:SetProperty(GOV_WINDOW_RECEIPT,receipt);
        culture:SetCivic(civic.Index,true);
        receipt.phase="SETTER_RETURNED";
        receipt.afterTotal=titles:GetGovernorPoints();
        receipt.afterSpent=titles:GetGovernorPointsSpent();
        receipt.owned=culture:HasCivic(civic.Index);
        receipt.status=receipt.owned and
            receipt.afterTotal==total+expected and
            receipt.afterSpent==spent and "OBSERVED" or "PENDING_NATIVE";
        Game:SetProperty(GOV_WINDOW_RECEIPT,receipt);
    end);
    if not ok then
        receipt.status="ERROR"; receipt.error="CALLBACK_FAILED";
        Game:SetProperty(GOV_WINDOW_RECEIPT,receipt);
        print("JAC_BOUNDARY CORE_GOV_WINDOW_CIVIC_ERROR "..tostring(err));
    end
    print("JAC_BOUNDARY CORE_GOV_WINDOW_CIVIC_RESULT "..R.Serialize(receipt));
end);

GameEvents.JAC_BoundaryReadTier1GovernmentWindowCivic.Add(function(controller,params)
    local prior=Game:GetProperty(GOV_WINDOW_READ);
    local attempt=Game:GetProperty(GOV_WINDOW_ATTEMPT);
    local receipt=Game:GetProperty(GOV_WINDOW_RECEIPT);
    local read={owner=controller,action=type(params)=="table" and params.Action or "",
        token=type(params)=="table" and params.Token or -1,
        turn=Game.GetCurrentGameTurn(),
        frame=type(params)=="table" and params.Frame or -1,
        publishEpoch=type(params)=="table" and params.PublishEpoch or -1,
        serial=(prior and prior.serial or 0)+1,status="ENTERED"};
    Game:SetProperty(GOV_WINDOW_READ,read);
    local ok,err=pcall(function()
        EnemyReceipt(controller,params,"PREPARE_TIER1_GOV_WINDOW_CIVIC");
        assert(params.Phase=="READ" and attempt and receipt and
            receipt.phase=="SETTER_RETURNED" and receipt.status~="ERROR" and
            receipt.owner==attempt.owner and receipt.token==attempt.token and
            receipt.index==attempt.index and receipt.civicType==attempt.civicType and
            receipt.turn==attempt.turn and receipt.beforeTotal==attempt.beforeTotal and
            receipt.beforeSpent==attempt.beforeSpent and
            params.OriginalOwner==attempt.owner and
            params.OriginalIndex==attempt.index and
            params.OriginalToken==attempt.token and
            params.OriginalTurn==attempt.turn and
            params.OriginalSpent==attempt.beforeSpent and
            params.OriginalSequence==attempt.sequence and
            params.CivicType==attempt.civicType and params.Token==attempt.token and
            params.Turn==attempt.turn,
            "Government-window Civic read is stale or mismatched");
        local player=Players[controller];
        local civic=assert(GameInfo.Civics[attempt.civicType],
            "Government-window Civic definition vanished");
        assert(civic.Index==attempt.index,"Government-window Civic index changed");
        read.originalToken=attempt.token; read.originalTurn=attempt.turn;
        read.index=civic.Index; read.owned=player:GetCulture():HasCivic(civic.Index);
        read.total=player:GetGovernors():GetGovernorPoints();
        read.spent=player:GetGovernors():GetGovernorPointsSpent();
        read.status=read.owned and read.total==attempt.beforeTotal+attempt.expectedTitleDelta and
            read.spent==attempt.beforeSpent and "OBSERVED" or "PENDING_NATIVE";
        if read.status=="OBSERVED" then
            Game:SetProperty(GOV_WINDOW_MARKER,{owner=controller,index=civic.Index,
                civicType=attempt.civicType,
                targetGovernment=attempt.targetGovernment,token=params.Token,
                originalToken=attempt.token,originalSequence=attempt.sequence,
                readSerial=read.serial,readFrame=read.frame,
                readPublishEpoch=read.publishEpoch,
                beforeTotal=attempt.beforeTotal,beforeSpent=attempt.beforeSpent,
                afterTotal=read.total,afterSpent=read.spent,
                commandTurn=attempt.turn,turn=read.turn});
        end
        Game:SetProperty(GOV_WINDOW_READ,read);
    end);
    if not ok then
        read.status="ERROR"; read.error="READ_FAILED";
        Game:SetProperty(GOV_WINDOW_READ,read);
        print("JAC_BOUNDARY CORE_GOV_WINDOW_CIVIC_READ_ERROR "..tostring(err));
    end
    print("JAC_BOUNDARY CORE_GOV_WINDOW_CIVIC_READ_RESULT "..R.Serialize(read));
end);

-- The official GovernmentScreen sends Culture:RequestChangeGovernment(hash)
-- from UI. Record one synchronized intent before that native request, so a
-- lost UI context never replays an uncertain government change.
GameEvents.JAC_BoundaryCapacityGovernmentIntent.Add(function(controller,params)
    local tier=type(params)=="table" and params.TargetTier or "";
    local intents=Game:GetProperty(CAPACITY_GOVERNMENT_INTENTS) or {};
    if intents[tier] then
        print("JAC_BOUNDARY CORE_CAPACITY_GOVERNMENT_INTENT_DUPLICATE_SUPPRESSED");
        return;
    end
    local intent={owner=controller,tier=tier,
        token=type(params)=="table" and params.Token or -1,
        turn=Game.GetCurrentGameTurn(),status="ENTERED"};
    intents[tier]=intent;
    Game:SetProperty(CAPACITY_GOVERNMENT_INTENTS,intents);
    print("JAC_BOUNDARY CORE_CAPACITY_GOVERNMENT_INTENT_ENTRY "..R.Serialize(intent));
    local ok,err=pcall(function()
        EnemyReceipt(controller,params,"CHANGE_CAPACITY_GOVERNMENT");
        EnemyConfig(controller);
        assert(tier=="Tier1" or tier=="Tier2",
            "unknown native capacity Government tier");
        local row=assert(GameInfo.Governments[params.TargetGovernment],
            "capacity Government definition missing");
        local nativeType=assert(GameInfo.Types[params.TargetGovernment],
            "capacity Government native type missing");
        local civic=assert(GameInfo.Civics[row.PrereqCivic],
            "capacity Government prerequisite Civic missing");
        local player=assert(Players[controller],"capacity Government actor missing");
        local culture=player:GetCulture();
        local sourceKey=tier=="Tier1" and GOV_WINDOW_ATTEMPT or TIER2_CIVIC_ATTEMPT;
        local receiptKey=tier=="Tier1" and GOV_WINDOW_RECEIPT or TIER2_CIVIC_RECEIPT;
        local source=assert(Game:GetProperty(sourceKey),
            "fresh official Civic attempt for Government change missing");
        local civicReceipt=assert(Game:GetProperty(receiptKey),
            "fresh official Civic receipt for Government change missing");
        local sourceCivic=assert(GameInfo.Civics[source.civicType],
            "Government-change source Civic definition missing");
        assert(source.owner==controller and source.turn==Game.GetCurrentGameTurn() and
            source.targetGovernment==row.GovernmentType and
            source.index==sourceCivic.Index and
            source.token==params.SourceCivicToken and
            source.turn==params.SourceCivicTurn and
            source.index==params.SourceCivicIndex and
            source.civicType==params.SourceCivicType and
            source.beforeTotal==params.SourceCivicBeforeTotal and
            source.expectedTitleDelta==params.SourceCivicDelta and
            source.expectedTitleDelta==OfficialCivicTitleDelta(source.civicType) and
            civicReceipt.token==source.token and civicReceipt.phase=="SETTER_RETURNED" and
            civicReceipt.status=="OBSERVED" and civicReceipt.owned and
            civicReceipt.afterTotal==source.beforeTotal+source.expectedTitleDelta and
            civicReceipt.afterTotal==params.BeforeTotal and
            civicReceipt.afterSpent==source.beforeSpent and
            civicReceipt.afterSpent==params.BeforeSpent and
            culture:HasCivic(sourceCivic.Index),
            "Government change does not follow this turn's native Civic completion");
        local nativeGov=assert(ExposedMembers.JAC_PersonaBoundaryGovernmentNativeRead,
            "UI native Government reader unavailable")(controller);
        assert(row.Tier==tier and row.Index==params.TargetIndex and
            nativeType.Hash==params.TargetHash and
            civic.Index==params.PrereqCivicIndex and culture:HasCivic(civic.Index) and
            nativeGov.owner==controller and nativeGov.turn==params.Turn and
            nativeGov.index==params.BeforeGovernment and
            nativeGov.governmentType==GameInfo.Governments[params.BeforeGovernment].GovernmentType and
            nativeGov.frame>=params.Frame and
            nativeGov.publishEpoch>=params.PublishEpoch and
            player:GetGovernors():GetGovernorPoints()==params.BeforeTotal and
            player:GetGovernors():GetGovernorPointsSpent()==params.BeforeSpent,
            "native capacity Government intent no longer matches the real actor");
        intent.targetGovernment=row.GovernmentType;
        intent.targetIndex=row.Index; intent.targetHash=nativeType.Hash;
        intent.prereqCivicIndex=civic.Index;
        intent.beforeGovernment=params.BeforeGovernment;
        intent.beforeTotal=params.BeforeTotal;
        intent.beforeSpent=params.BeforeSpent;
        intent.sourceCivicType=source.civicType;
        intent.sourceCivicIndex=source.index;
        intent.sourceCivicToken=source.token;
        intent.sourceCivicTurn=source.turn;
        intent.sourceCivicBeforeTotal=source.beforeTotal;
        intent.sourceCivicDelta=source.expectedTitleDelta;
        intent.frame=params.Frame; intent.publishEpoch=params.PublishEpoch;
        intent.status="PREPARED";
        Game:SetProperty(CAPACITY_GOVERNMENT_INTENTS,intents);
    end);
    if not ok then
        intent.status="ERROR"; intent.error="INTENT_FAILED";
        Game:SetProperty(CAPACITY_GOVERNMENT_INTENTS,intents);
        print("JAC_BOUNDARY CORE_CAPACITY_GOVERNMENT_INTENT_ERROR "..tostring(err));
    end
    print("JAC_BOUNDARY CORE_CAPACITY_GOVERNMENT_INTENT_RESULT "..R.Serialize(intent));
end);

GameEvents.JAC_BoundaryCapacityGovernmentProbe.Add(function(controller,params)
    EnemyReceipt(controller,params,"PROBE_CAPACITY_GOVERNMENT_API");
    EnemyConfig(controller);
    local culture=Players[controller]:GetCulture();
    local kind=type(culture.GetCurrentGovernment);
    local result={owner=controller,token=params.Token,turn=Game.GetCurrentGameTurn(),
        getter=kind,status="API_UNAVAILABLE",
        uiBridge=type(ExposedMembers.JAC_PersonaBoundaryGovernmentNativeRead),
        civicModifiers=type(GameInfo.CivicModifiers),
        modifierArguments=type(GameInfo.ModifierArguments),
        dynamicModifiers=type(GameInfo.DynamicModifiers),
        civicPrereqs=type(GameInfo.CivicPrereqs)};
    if kind=="function" then
        local ok,index=pcall(culture.GetCurrentGovernment,culture);
        if ok and type(index)=="number" then
            result.index=index; result.status="OBSERVED";
        else
            result.status="READ_FAILED";
        end
    end
    Game:SetProperty(CAPACITY_GOVERNMENT_API_PROBE,result);
    print("JAC_BOUNDARY CORE_CAPACITY_GOVERNMENT_API_PROBE "..R.Serialize(result));
end);

-- Classify the official UI Government request independently of the screen's
-- CivicCompletedThisTurn convenience gate. This records intent only; Gameplay
-- never sets a Government or invents a policy slot.
GameEvents.JAC_BoundaryClassifyCapacityGovernmentIntent.Add(function(controller,params)
    if Game:GetProperty(GOV_CLASSIFY_INTENT) then
        print("JAC_BOUNDARY CORE_GOV_CLASSIFY_DUPLICATE_SUPPRESSED");
        return;
    end
    local receipt={owner=controller,token=type(params)=="table" and params.Token or -1,
        turn=Game.GetCurrentGameTurn(),status="ENTERED"};
    Game:SetProperty(GOV_CLASSIFY_INTENT,receipt);
    local ok,err=pcall(function()
        EnemyReceipt(controller,params,"CLASSIFY_CAPACITY_GOVERNMENT");
        EnemyConfig(controller);
        local target=assert(GameInfo.Governments["GOVERNMENT_OLIGARCHY"],
            "retained Oligarchy Government missing");
        local nativeType=assert(GameInfo.Types[target.GovernmentType],
            "native Oligarchy type missing");
        local prerequisite=assert(GameInfo.Civics[target.PrereqCivic],
            "Oligarchy Civic prerequisite missing");
        local player=assert(Players[controller],"classification actor missing");
        local culture=player:GetCulture();
        local bridge=assert(ExposedMembers.JAC_PersonaBoundaryGovernmentNativeRead,
            "UI native Government reader missing")(controller);
        assert(target.Tier=="Tier1" and params.TargetGovernment==target.GovernmentType and
            params.TargetIndex==target.Index and params.TargetHash==nativeType.Hash and
            params.PrereqCivicIndex==prerequisite.Index and
            culture:HasCivic(prerequisite.Index) and
            params.Turn==24 and params.BeforeTotal==1 and params.BeforeSpent==0 and
            params.BeforeCivicCompleted==false and
            bridge.owner==controller and bridge.turn==params.Turn and
            bridge.index==params.BeforeGovernment and
            bridge.governmentType=="GOVERNMENT_CHIEFDOM" and bridge.tier=="NONE" and
            bridge.frame>=params.Frame and bridge.publishEpoch>=params.PublishEpoch and
            player:GetGovernors():GetGovernorPoints()==params.BeforeTotal and
            player:GetGovernors():GetGovernorPointsSpent()==params.BeforeSpent,
            "classification baseline differs from the saved native actor");
        receipt.targetGovernment=target.GovernmentType;
        receipt.targetIndex=target.Index; receipt.targetHash=nativeType.Hash;
        receipt.beforeGovernment=bridge.index;
        receipt.beforeTotal=params.BeforeTotal;
        receipt.beforeSpent=params.BeforeSpent;
        receipt.frame=params.Frame; receipt.publishEpoch=params.PublishEpoch;
        receipt.status="PREPARED";
        Game:SetProperty(GOV_CLASSIFY_INTENT,receipt);
    end);
    if not ok then
        receipt.status="ERROR"; receipt.error="INTENT_FAILED";
        Game:SetProperty(GOV_CLASSIFY_INTENT,receipt);
        print("JAC_BOUNDARY CORE_GOV_CLASSIFY_INTENT_ERROR "..tostring(err));
    end
    print("JAC_BOUNDARY CORE_GOV_CLASSIFY_INTENT "..R.Serialize(receipt));
end);

GameEvents.JAC_BoundaryReadCapacityGovernmentClassification.Add(function(controller,params)
    local previous=Game:GetProperty(GOV_CLASSIFY_READ);
    if previous then
        print("JAC_BOUNDARY CORE_GOV_CLASSIFY_READ_DUPLICATE_SUPPRESSED");
        return;
    end
    local intent=Game:GetProperty(GOV_CLASSIFY_INTENT);
    local read={owner=controller,token=type(params)=="table" and params.Token or -1,
        turn=Game.GetCurrentGameTurn(),frame=type(params)=="table" and params.Frame or -1,
        publishEpoch=type(params)=="table" and params.PublishEpoch or -1,
        status="ENTERED"};
    Game:SetProperty(GOV_CLASSIFY_READ,read);
    local ok,err=pcall(function()
        EnemyReceipt(controller,params,"READ_CAPACITY_GOVERNMENT_CLASSIFICATION");
        assert(intent and intent.status=="PREPARED" and intent.owner==controller and
            intent.token==params.OriginalToken and intent.turn==params.OriginalTurn and
            params.Turn==intent.turn+1 and params.Token>intent.token and
            params.TargetGovernment==intent.targetGovernment and
            params.TargetIndex==intent.targetIndex and
            params.TargetHash==intent.targetHash and
            params.BeforeGovernment==intent.beforeGovernment and
            params.BeforeTotal==intent.beforeTotal and
            params.BeforeSpent==intent.beforeSpent,
            "classification read does not match the original attempt");
        local bridge=assert(ExposedMembers.JAC_PersonaBoundaryGovernmentNativeRead,
            "UI native Government reader unavailable at classification read")(controller);
        assert(bridge.owner==controller and bridge.turn==params.Turn and
            bridge.frame>=params.Frame and bridge.publishEpoch>=params.PublishEpoch,
            "classification native Government read is stale");
        local titles=Players[controller]:GetGovernors();
        read.originalToken=intent.token; read.originalTurn=intent.turn;
        read.index=bridge.index; read.governmentType=bridge.governmentType;
        read.tier=bridge.tier; read.nativeFrame=bridge.frame;
        read.nativePublishEpoch=bridge.publishEpoch;
        read.nativeSource="UI_NATIVE_CULTURE";
        read.total=titles:GetGovernorPoints();
        read.spent=titles:GetGovernorPointsSpent();
        read.status="OBSERVED";
        Game:SetProperty(GOV_CLASSIFY_READ,read);
    end);
    if not ok then
        read.status="ERROR"; read.error="READ_FAILED";
        Game:SetProperty(GOV_CLASSIFY_READ,read);
        print("JAC_BOUNDARY CORE_GOV_CLASSIFY_READ_ERROR "..tostring(err));
    end
    print("JAC_BOUNDARY CORE_GOV_CLASSIFY_READ "..R.Serialize(read));
end);

-- This callback only reads the native Government after a real turn. It never
-- invokes RequestChangeGovernment or writes a synthetic Government state.
GameEvents.JAC_BoundaryCapacityGovernmentRead.Add(function(controller,params)
    local prior=Game:GetProperty(CAPACITY_GOVERNMENT_READ);
    local read={owner=controller,token=type(params)=="table" and params.Token or -1,
        turn=Game.GetCurrentGameTurn(),
        frame=type(params)=="table" and params.Frame or -1,
        publishEpoch=type(params)=="table" and params.PublishEpoch or -1,
        serial=(prior and prior.serial or 0)+1,status="ENTERED"};
    Game:SetProperty(CAPACITY_GOVERNMENT_READ,read);
    print("JAC_BOUNDARY CORE_CAPACITY_GOVERNMENT_READ_ENTRY "..R.Serialize(read));
    local ok,err=pcall(function()
        EnemyReceipt(controller,params,"CHANGE_CAPACITY_GOVERNMENT");
        assert(params.Phase=="GOV_READ" and
            (params.TargetTier=="Tier1" or params.TargetTier=="Tier2"),
            "capacity Government read phase or tier changed");
        local intents=Game:GetProperty(CAPACITY_GOVERNMENT_INTENTS) or {};
        local intent=assert(intents[params.TargetTier],
            "native capacity Government intent missing");
        assert(intent.status=="PREPARED" and intent.owner==controller and
            intent.token==params.OriginalToken and
            intent.turn==params.OriginalTurn and
            intent.targetGovernment==params.TargetGovernment and
            intent.targetIndex==params.TargetIndex and
            intent.targetHash==params.TargetHash and
            intent.beforeGovernment==params.BeforeGovernment and
            intent.beforeTotal==params.BeforeTotal and
            intent.beforeSpent==params.BeforeSpent and
            intent.sourceCivicToken==params.SourceCivicToken and
            intent.sourceCivicType==params.SourceCivicType and
            intent.sourceCivicIndex==params.SourceCivicIndex and
            intent.sourceCivicTurn==params.SourceCivicTurn and
            intent.sourceCivicBeforeTotal==params.SourceCivicBeforeTotal and
            intent.sourceCivicDelta==params.SourceCivicDelta and
            params.Turn==intent.turn+1 and params.Token>intent.token,
            "native capacity Government read is stale or changed");
        local player=assert(Players[controller],"capacity Government read actor missing");
        local culture=player:GetCulture();
        local nativeGov=assert(ExposedMembers.JAC_PersonaBoundaryGovernmentNativeRead,
            "UI native Government reader unavailable at read")(controller);
        assert(nativeGov.owner==controller and nativeGov.turn==params.Turn and
            nativeGov.frame>=params.Frame and
            nativeGov.publishEpoch>=params.PublishEpoch,
            "native Government UI bridge is stale at read");
        read.originalToken=intent.token; read.originalTurn=intent.turn;
        read.tier=intent.tier;
        read.governmentIndex=nativeGov.index;
        read.governmentType=nativeGov.governmentType;
        read.governmentTier=nativeGov.tier;
        read.nativeFrame=nativeGov.frame;
        read.nativePublishEpoch=nativeGov.publishEpoch;
        read.nativeSource="UI_NATIVE_CULTURE";
        read.total=player:GetGovernors():GetGovernorPoints();
        read.spent=player:GetGovernors():GetGovernorPointsSpent();
        local sourceCivic=assert(GameInfo.Civics[intent.sourceCivicType],
            "Government-change source Civic missing at read");
        read.sourceCivicOwned=culture:HasCivic(sourceCivic.Index);
        read.sourceCivicIndex=sourceCivic.Index;
        read.status="OBSERVED";
        Game:SetProperty(CAPACITY_GOVERNMENT_READ,read);
    end);
    if not ok then
        read.status="ERROR"; read.error="READ_FAILED";
        Game:SetProperty(CAPACITY_GOVERNMENT_READ,read);
        print("JAC_BOUNDARY CORE_CAPACITY_GOVERNMENT_READ_ERROR "..tostring(err));
    end
    print("JAC_BOUNDARY CORE_CAPACITY_GOVERNMENT_READ_RESULT "..R.Serialize(read));
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
    assert(params.Unassigned==0 or params.Unassigned==1,
        "native assignment observation missing at recovery capture");
    if params.Unassigned==1 then
        assert(native.cityOwner==nil and native.cityID==nil,
            "native Governor was assigned after the UI unassigned observation");
    else
        assert(native.cityOwner==c.owner and native.cityID==c.cityID,
            "native Governor assignment changed before recovery capture");
    end
    local nodes={};
    for node in GameInfo.JAC_PersonaTreeNodes() do
        if node.LeaderType==c.leader and R.Property(c.leader,"NODE_"..node.NodeType)==1 then
            nodes[node.NodeType]=1;
        end
    end
    Game:SetProperty("JAC_BOUNDARY_ENEMY_RECOVERY",{merit=R.Property(c.leader,"MERIT") or 0,
        nodesOwned=nodes,spent=Players[c.owner]:GetGovernors():GetGovernorPointsSpent(),
        remaining=remaining,unassignedAtCapture=params.Unassigned,
        turn=Game.GetCurrentGameTurn()});
end);

-- Reserve the one native research request before it is sent from UI. The
-- scenario saves this marker first; a reloaded UI has no matching local
-- armed token and cannot submit the request a second time.
GameEvents.JAC_BoundaryThirdSpyCivicIntent.Add(function(controller,params)
    EnemyReceipt(controller,params,"THIRD_SPY_CIVIC_INTENT");
    local c=EnemyConfig(controller);
    assert(c.cityID and not Game:GetProperty(THIRD_CIVIC_INTENT),
        "third spy Civic intent already reserved");
    local retry=assert(Game:GetProperty(RETRY),"first failed mission archive missing");
    local event=assert(Game:GetProperty("JAC_BOUNDARY_ENEMY_MISSION"),
        "second failed mission event missing");
    local spy=assert(Game:GetProperty("JAC_BOUNDARY_SPY"),
        "second failed spy reference missing");
    local native=NativeEnemy(c);
    assert(retry.current==2 and retry.archive and
        retry.archive.eventID==0 and retry.archive.eventTurn==23 and
        event.attacker==controller and event.id==1 and event.turn==45 and
        spy.owner==controller and spy.id==851976 and
        native.appointed and native.base and native.established and
        native.neutralized==0 and native.cityOwner==c.owner and
        native.cityID==c.cityID and params.EventID==event.id and
        params.EventTurn==event.turn and params.SpyID==spy.id and
        params.BeforeCapacity==2,
        "native failed attempts or Governor changed before Civic intent");
    local civic=assert(GameInfo.Civics["CIVIC_JAC_SENGOKU_NANBAN_ENVOYS"],
        "test-only native spy grant Civic missing");
    local prereq=assert(GameInfo.Civics["CIVIC_JAC_SENGOKU_PORT_MERCHANT_COUNCILS"],
        "Nanban Civic prerequisite missing");
    local culture=Players[controller]:GetCulture();
    assert(civic.Index==params.CivicIndex and civic.Hash==params.CivicHash and
        not culture:HasCivic(civic.Index) and culture:HasCivic(prereq.Index),
        "native Civic identity or research prerequisites changed");
    local grants=0;
    for row in GameInfo.CivicModifiers() do
        if row.CivicType==civic.CivicType and row.ModifierId=="CIVIC_GRANT_SPY" then
            grants=grants+1;
        end
    end
    assert(grants==1,"test-only native spy grant was not installed");
    Game:SetProperty(THIRD_CIVIC_INTENT,{owner=controller,status="RESERVED",
        token=params.Token,turn=params.Turn,frame=params.Frame,
        publishEpoch=params.PublishEpoch,civicIndex=civic.Index,
        civicHash=civic.Hash,eventID=event.id,eventTurn=event.turn,
        spyID=spy.id,capacitySource="UI",beforeCapacity=params.BeforeCapacity});
    print("JAC_BOUNDARY THIRD_SPY_CIVIC_INTENT "..
        R.Serialize(Game:GetProperty(THIRD_CIVIC_INTENT)));
end);

ExposedMembers.JAC_PersonaBoundaryGameplayRead = ReadGameplay;
if R.Third then R.Third.InstallCore({config=EnemyConfig,native=NativeEnemy}); end
print("JAC_PersonaBoundary_Fixture: ready (test only; no automatic mutations)");
