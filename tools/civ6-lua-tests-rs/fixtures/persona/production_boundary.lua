-- Local support for the four-merit-source boundary test. This fixture only
-- drives the current persona Gameplay/UI Lua loaded by the Rust harness.
DATE = 'LEADER_JAC_DATE_TANEMUNE'

function SETTLED_DATE()
    local api=ExposedMembers.JAC_LeaderPersona
    assert(api.Join(0,1,'CALM','ASTUTE')=='JOINED')
    Players[0]:GetGovernors():ChangeGovernorPoints(api.EvolutionCost(DATE))
    JAC_EvolveLeader(DATE)
    DRAIN()
    assert(Game:GetProperty('JAC_PERSONA_'..DATE..'_STATE')=='GOVERNOR')
    local row=GameInfo.JAC_LeaderPersonas[DATE]
    local governor=NATIVE_GOVS[0][GameInfo.Governors[row.GovernorType].Hash]
    governor.city=CITIES[0][1]
    return governor
end

function SET_BOUNDARY_PHASE(governor, phase)
    if phase=='NEUTRALIZED' then
        governor.city=nil
        governor.established=false
        governor.neutralized=3
    elseif phase=='RECOVERED' then
        governor.city=nil
        governor.established=false
    elseif phase=='ESTABLISHING' then
        governor.established=false
    elseif phase=='LOST' then
        local old=CITIES[0][1]
        old.GetOwner=function() return 2 end
        old.markerOverrides['JAC_PERSONA_SETTLED_'..DATE]=1
        CITIES[0][1]=nil
        CITIES[2][99]=old
        governor.city=nil
        governor.established=false
    else
        error('unknown boundary phase')
    end
end

local function warunit(owner,id,x,dead)
    return {GetID=function() return id end,GetOwner=function() return owner end,
        GetX=function() return x end,GetY=function() return 0 end,
        IsDead=function() return dead end,IsDelayedDeath=function() return false end}
end

function FIRE_FOUR_MERIT_SOURCES()
    FIRE('PlayerTurnStarted',0)
    FIRE('BuildingConstructed',0,1,0,100,true)
    NATIVE_UNITS[0][800]=warunit(0,800,1,false)
    NATIVE_UNITS[1][999]=warunit(1,999,3,true)
    FIRE('OnCombatOccurred',0,800,1,999)
    Players[0].allies[1]=true
    FIREUI('DiplomacyRelationshipChanged',0,1)
end

function FIRE_POSITIVE_CONTROL(governor)
    governor.city=CITIES[0][2]
    governor.established=true
    governor.neutralized=0
    TURN=3
    FIRE('PlayerTurnStarted',0)
    FIRE('BuildingConstructed',0,2,1,300,true)
end
