function store()
    local data = {}
    return function(_, k) return data[k] end, function(_, k, v) data[k] = v end
end
Game = { GetLocalPlayer=function() return LOCAL end, GetCurrentGameTurn=function() return TURN end }
Game.GetProperty, Game.SetProperty = store()
LOCAL, TURN, SPAWNS, DESTROYS, FAIL_SPAWN, FAIL_DESTROY = 0, 1, 0, 0, false, false
REJECT_PROMOTION_STEP = nil
function collection(rows, key)
    local t = {}
    for _,r in ipairs(rows) do t[r[key]]=r; if r.Index then t[r.Index]=r end end
    return setmetatable(t, {__call=function()
        local i=0; return function() i=i+1; return rows[i] end
    end})
end
local names = {'ODA_NOBUHIDE','DATE_TANEMUNE','HOJO_UJITSUNA'}
local civs = {'ODA','DATE','HOJO'}
ROWS, INDIVIDUALS, GOVDEFS, PROMOS = {}, {}, {}, {}
for i,n in ipairs(names) do
    local leader='LEADER_JAC_'..n
    local row={LeaderType=leader, CivilizationType='CIVILIZATION_JAC_'..civs[i],
        Name='LOC_'..leader..'_NAME', PortraitIcon='ICON_'..leader,
        PersonType='GREAT_PERSON_INDIVIDUAL_JAC_PERSONA_'..n, GovernorType='GOVERNOR_JAC_PERSONA_'..n,
        BasePromotionType='GOVERNOR_PROMOTION_JAC_PERSONA_'..n..'_BASE',
        Personality=i==1 and 'BOLD' or 'CALM', Trait=i==3 and 'DEVOUT' or 'ASTUTE',
        EvolutionCost=i==1 and 1 or 2}
    ROWS[i]=row
    INDIVIDUALS[i]={GreatPersonIndividualType=row.PersonType, GreatPersonClassType="GREAT_PERSON_CLASS_JAC_PERSONA", Index=i+10, Hash=3000+i, EraType="ERA_ANCIENT", Name=row.Name}
    GOVDEFS[i]={GovernorType=row.GovernorType, Index=i+20, Hash=1000+i, Name=row.Name}
    PROMOS[#PROMOS+1]={GovernorPromotionType=row.BasePromotionType, Index=i+30, Hash=2000+i}
    for step=1,2 do
        PROMOS[#PROMOS+1]={GovernorPromotionType=row.BasePromotionType..'_TITLE_'..step,
            Index=40+i*2+step, Hash=2100+i*2+step}
    end
end
GameInfo={ Eras=collection({{EraType="ERA_ANCIENT",Hash=5000}},"EraType"),
    GreatPersonClasses=collection({{GreatPersonClassType="GREAT_PERSON_CLASS_JAC_PERSONA",UnitType="UNIT_JAC_PERSONA",Hash=4000}},"GreatPersonClassType"),
    Units=collection({{UnitType="UNIT_JAC_PERSONA",Index=99}},"UnitType"), JAC_LeaderPersonas=collection(ROWS,'LeaderType'),
    GreatPersonIndividuals=collection(INDIVIDUALS,'GreatPersonIndividualType'),
    Governors=collection(GOVDEFS,'GovernorType'), GovernorPromotions=collection(PROMOS,'GovernorPromotionType') }
PlayerConfigurations, Players = {}, {}
NATIVE_UNITS, NATIVE_GOVS, CITIES, GRANTS, NATIVE_APPOINTABLE = {}, {}, {}, {}, {}
function members(rows)
    local list={}; for _,v in pairs(rows) do list[#list+1]=v end
    table.sort(list,function(a,b) return a:GetID()<b:GetID() end)
    local i=0; return function() i=i+1; if list[i] then return i,list[i] end end
end
for i,row in ipairs(ROWS) do
    local id=i-1
    local config={leader=row.LeaderType, civ=row.CivilizationType}
    config.GetLeaderTypeName=function() return config.leader end
    config.GetCivilizationTypeName=function() return config.civ end
    PlayerConfigurations[id]=config
    NATIVE_UNITS[id], NATIVE_GOVS[id], GRANTS[id], CITIES[id]={},{},{},{}
    NATIVE_APPOINTABLE[id]={}
    local p={points=0,spent=0,alive=true,major=true,active=true,human=true,allies={}}
    p.GetID=function() return id end
    p.IsAlive=function() return p.alive end; p.IsMajor=function() return p.major end
    p.IsTurnActive=function() return p.active end
    p.IsHuman=function() return p.human end
    p.GetDiplomacy=function() return {HasAllied=function(_,other) return p.allies[other]==true end} end
    p.GetProperty,p.SetProperty=store()
    p.GetUnits=function() return {Members=function() return members(NATIVE_UNITS[id]) end,
        FindID=function(_,unit) return NATIVE_UNITS[id][unit] end,
        Destroy=function(_,u) DESTROYS=DESTROYS+1; if not FAIL_DESTROY then NATIVE_UNITS[id][u:GetID()]=nil end end} end
    for j=1,2 do
        local cityid=id*10+j
        local city={markerOverrides={}, GetID=function() return cityid end, GetOwner=function() return id end,
            GetX=function() return j end, GetY=function() return id end}
        city.GetProperty=function(_,key)
            if city.markerOverrides[key] ~= nil then return city.markerOverrides[key] end
            return MARKER_VALUE(id,city,key)
        end
        CITIES[id][cityid]=city
    end
    p.GetCities=function() return {GetCapitalCity=function() return CITIES[id][id*10+1] end,
        Members=function() return members(CITIES[id]) end} end
    p.GetGovernors=function() return {
        GetGovernor=function(_,hash) return NATIVE_GOVS[id][hash] end,
        HasGovernor=function(_,hash) return NATIVE_GOVS[id][hash]~=nil end,
        CanAppoint=function() return p.points>p.spent end,
        CanEverAppointGovernor=function(_,hash)
            assert(hash>=1000, 'per-type getter expects a governor Hash')
            return NATIVE_APPOINTABLE[id][hash]~=false
        end,
        CanAppointGovernor=function(_,hash)
            assert(hash>=1000, 'per-type getter expects a governor Hash')
            return NATIVE_APPOINTABLE[id][hash]~=false and NATIVE_GOVS[id][hash]==nil and p.points>p.spent
        end,
        CanPromoteGovernor=function(_,hash) return NATIVE_GOVS[id][hash]~=nil and p.points>p.spent end,
        CanAssignGovernor=function(_,hash,city) return NATIVE_GOVS[id][hash]~=nil and city:GetOwner()==id end,
        ChangeGovernorPoints=function(_,delta) GRANTS[id][#GRANTS[id]+1]=delta; p.points=p.points+delta end,
        GetGovernorPoints=function() return p.points end, GetGovernorPointsSpent=function() return p.spent end
    } end
    Players[id]=p
end
PlayerManager={GetAliveMajorIDs=function() return {0,1,2} end}
PLOTS={}
Map={GetPlot=function(x,y)
    local key=x..'_'..y
    if not PLOTS[key] then local p={};p.GetProperty,p.SetProperty=store();PLOTS[key]=p end
    return PLOTS[key]
end,GetPlotDistance=function(x,y,ox,oy) return math.max(math.abs(x-ox),math.abs(y-oy)) end}
CityManager={GetCity=function(owner,id) return CITIES[owner] and CITIES[owner][id] end}
local nextUnit=0
Game.GetGreatPeople=function() return { CreatePerson=function() error('Tuner-only creation forbidden') end,
    GrantPerson=function(_,hash,class,era,cost,owner,claim)
    SPAWNS=SPAWNS+1; if FAIL_SPAWN then return end
    local individual
    for _,row in ipairs(INDIVIDUALS) do if row.Hash==hash then individual=row.Index end end
    assert(individual, 'individual HASH required')
    assert(class==4000 and era==5000 and cost==0 and claim==false, 'native grant arguments required')
    nextUnit=nextUnit+1; local id=nextUnit
    local u={ individual=individual, GetID=function() return id end,
        GetOwner=function() return owner end, GetType=function() return 99 end }
    u.GetGreatPerson=function()
        if GAMECORE_UNIT_READS and not UI_NATIVE_READ then return {GetIndividual=function() return u.individual end} end
        return {IsGreatPerson=function() return true end, GetIndividual=function() return u.individual end}
    end
    NATIVE_UNITS[owner][id]=u
    -- Deliberately NO return: neither creation nor removal is used as an object-returning API.
end } end
function NEWGOV(owner,leader,paid)
    local row=GameInfo.JAC_LeaderPersonas[leader]; local def=GameInfo.Governors[row.GovernorType]
    local g={promotions={},established=true,neutralized=0}
    g.promotions[GameInfo.GovernorPromotions[row.BasePromotionType].Hash]=true
    if paid~=false then
        local surcharge=(Game:GetProperty('JAC_PERSONA_'..leader..'_EVOLUTION_SURCHARGE') or 0)
        for step=1,surcharge do
            g.promotions[GameInfo.GovernorPromotions[row.BasePromotionType..'_TITLE_'..step].Hash]=true
        end
    end
    g.GetType=function() return def.Index end
    g.IsEstablished=function() return g.established end
    g.GetNeutralizedTurns=function() return g.neutralized end
    g.GetAssignedCity=function() return g.city end
    g.HasPromotion=function(_,hash) return g.promotions[hash]==true end
    NATIVE_GOVS[owner][def.Hash]=g; return g
end
-- Simulated modifier activation, separate from the gameplay Lua under test.
function MARKER_VALUE(owner,city,key)
    for _,row in ipairs(ROWS) do
        if key=='JAC_PERSONA_SETTLED_'..row.LeaderType then
            local def=GameInfo.Governors[row.GovernorType]
            local governor=NATIVE_GOVS[owner][def.Hash]
            local base=GameInfo.GovernorPromotions[row.BasePromotionType]
            if governor and governor.city==city and governor.established and governor.neutralized==0
                and governor.promotions[base.Hash] then return 1 end
            return nil
        end
    end
end
ExposedMembers={}
function events()
    local handlers={}
    local bus=setmetatable({}, {__index=function(t,k)
        -- GameplaySetActivePlayer is not a UI event in the installed game.
        if k=='GameplaySetActivePlayer' then return nil end
        local ev=setmetatable({Add=function(f) handlers[k]=handlers[k] or {}; table.insert(handlers[k],f) end},
            {__call=function(_,...) for _,f in ipairs(handlers[k] or {}) do f(...) end end})
        rawset(t,k,ev); return ev
    end})
    return bus,function(name,...) for _,f in ipairs(handlers[name] or {}) do f(...) end end
end
GameEvents,FIRE=events(); Events,FIREUI=events(); LuaEvents=events()
function RESET_CONTEXTS() GameEvents,FIRE=events(); Events,FIREUI=events(); LuaEvents=events() end
function print() end
function include() end
Mouse={eLClick=1}
Locale={Lookup=function(s) return s end, ToUpper=function(s) return s end}

function control()
    local c={hidden=true,disabled=false,text='',callbacks={}}
    c.SetText=function(_,v) c.text=v end; c.GetText=function() return c.text end
    c.SetIcon=function(_,v,size) c.icon=v;c.size=size end
    c.SetHide=function(_,v) c.hidden=v end; c.SetDisabled=function(_,v) c.disabled=v end
    c.RegisterCallback=function(_,event,f) c.callbacks[event]=f end
    c.CalculateSize=function() end; c.CalculateInternalSize=function() end
    c.DoAutoSize=function() end; c.IsHidden=function() return c.hidden end
    c.SetToolTipString=function(_,v) c.tooltip=v end
    c.SetScrollValue=function(_,v) c.scroll=v end
    return c
end
Controls={}
for _,name in ipairs({'Rows','PersonaPanel','PersonaToggle','Governors','Categories','Nodes','NodesScroll',
    'Name','Portrait','Merit','Commitment','Selection','Effect','Result','Confirmation','ConfirmText','Locks',
    'Confirm','Cancel','Close','NextPerson'}) do Controls[name]=control() end
ContextPtr=control(); ContextPtr.SetInputHandler=function(_,f) INPUT=f end
Keys={VK_F8=119,VK_TAB=9,VK_UP=38,VK_DOWN=40,VK_LEFT=37,VK_RIGHT=39,VK_RETURN=13,VK_ESCAPE=27}
KeyEvents={KeyUp=1}
function INPUT_KEY(key) return {GetKey=function() return key end,GetMessageType=function() return 1 end} end
VIEWS,CATEGORY_VIEWS,TREE_VIEWS={},{},{}
InstanceManager={new=function(_,kind)
    local function list()
        if kind=='PersonaRow' then return VIEWS elseif kind=='PersonaCategory' then return CATEGORY_VIEWS else return TREE_VIEWS end
    end
    return {ResetInstances=function()
        if kind=='PersonaRow' then VIEWS={} elseif kind=='PersonaCategory' then CATEGORY_VIEWS={} else TREE_VIEWS={} end
    end,GetInstance=function()
        local v={}
        for _,n in ipairs({'Root','Portrait','Name','Status','Evolve','Merit','Tree','Route','Node','Effect'}) do v[n]=control() end
        v.Portrait.SetIcon=function(_,icon) v.icon=icon end
        v.Name.SetText=function(_,name) v.name=name end
        v.Evolve.SetDisabled=function(_,disabled) v.disabled=disabled end
        v.Status.SetText=function(_,status) v.status=status end
        local items=list();items[#items+1]=v;return v
    end}
end}
PlayerOperations={EXECUTE_SCRIPT=1,APPOINT_GOVERNOR=2,ASSIGN_GOVERNOR=3,PROMOTE_GOVERNOR=4,
    PARAM_GOVERNOR_TYPE='governor',PARAM_PLAYER_ONE='cityowner',PARAM_CITY_DEST='city',
    PARAM_GOVERNOR_PROMOTION_TYPE='promotion'}
REQUESTS={}
UI={RequestPlayerOperation=function(owner,op,params) REQUESTS[#REQUESTS+1]={owner=owner,op=op,params=params} end}
function STEP()
    local req=table.remove(REQUESTS,1); if not req then return false end
    local owner,p,params=req.owner,Players[req.owner],req.params
    if req.op==1 then FIRE(params.OnStart,owner,params)
    elseif req.op==2 then
        local def=assert(GameInfo.Governors[params.governor], 'operation expects INDEX, not Hash')
        assert(p.points>p.spent)
        local leader
        for _,r in ipairs(ROWS) do if r.GovernorType==def.GovernorType then leader=r.LeaderType end end
        NEWGOV(owner,leader,false); p.spent=p.spent+1; FIREUI('GovernorChanged',owner,def.Index)
    elseif req.op==3 then
        local def=assert(GameInfo.Governors[params.governor])
        assert(params.cityowner==owner and CITIES[owner][params.city])
        NATIVE_GOVS[owner][def.Hash].city=CITIES[owner][params.city]
    elseif req.op==4 then
        local def=assert(GameInfo.Governors[params.governor]); local promo=assert(GameInfo.GovernorPromotions[params.promotion])
        local native=NATIVE_GOVS[owner][def.Hash]
        local leader
        for _,r in ipairs(ROWS) do if r.GovernorType==def.GovernorType then leader=r.LeaderType end end
        if p.points>p.spent and native and not native.promotions[promo.Hash] then
            local prior=promo.GovernorPromotionType:match('_TITLE_(%d)$')
            if prior then
                prior=tonumber(prior)
                assert(promo.GovernorPromotionType==GameInfo.JAC_LeaderPersonas[leader].BasePromotionType..'_TITLE_'..prior)
                if prior~=REJECT_PROMOTION_STEP and (prior==1 or native.promotions[GameInfo.GovernorPromotions[
                    GameInfo.JAC_LeaderPersonas[leader].BasePromotionType..'_TITLE_'..(prior-1)].Hash]) then
                    native.promotions[promo.Hash]=true; p.spent=p.spent+1
                end
            end
        end
    end
    FIREUI('GameCoreEventPublishComplete'); return true
end
function DRAIN() for i=1,20 do if not STEP() then return end end; error('request loop') end
