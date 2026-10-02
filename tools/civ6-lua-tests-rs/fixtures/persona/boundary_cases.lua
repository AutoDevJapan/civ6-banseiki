-- Mutations for the independent boundary engine. The production MOD Lua is
-- loaded separately from the current worktree by the Rust harness.
BOUNDARY_CASES = {}
local C = BOUNDARY_CASES

function C.spy(owner, id)
    makeSpy(owner, id)
    Game:SetProperty('JAC_BOUNDARY_SPY', {owner=owner, id=id})
end

function C.mission(initial, plot, turn, escape)
    HISTORY[1][0] = {Operation=0, PlotIndex=plot, CompletionTurn=turn,
        InitialResult=initial, EscapeResult=escape}
end

function C.missing_history()
    stop(3)
    Players[1].GetDiplomacy=function()
        return {GetMission=function() error('enemy history unavailable') end}
    end
end

function C.merit_disagreement()
    ExposedMembers.JAC_PersonaBoundaryGameplayRead=function()
        return {merit=13,cities={}}
    end
end

function C.inventory_unavailable()
    GameEffects.GetModifiers=function() return nil end
end

function C.activity_unavailable()
    GameEffects.GetModifiers=function() return {100} end
    GameEffects.GetModifierActive=function() return nil end
end

function C.foreign_active_only()
    GameEffects.GetModifierActive=function(id) return id==200 end
end

function C.orphan_effects()
    lose()
    EFFECT_TARGET=true
    GameEffects.GetModifierSubjects=function()
        return {{owner=0, name='Old cached city name'}}
    end
end

function C.other_player_effects()
    GameEffects.GetModifiers=function() return {100,101,102,200} end
    local definition=GameEffects.GetModifierDefinition
    local owner=GameEffects.GetModifierOwner
    local subjects=GameEffects.GetModifierSubjects
    GameEffects.GetModifierDefinition=function(id)
        return id==200 and {Id=NODE} or definition(id)
    end
    GameEffects.GetModifierOwner=function(id)
        return id==200 and {owner=1,name='Other governor'} or owner(id)
    end
    GameEffects.GetModifierSubjects=function(id)
        return id==200 and {FOREIGN} or subjects(id)
    end
end

function C.target_governor()
    Players[1].GetGovernors=function()
        return {
            HasGovernor=function() return true end,
            GetGovernor=function()
                return {
                    GetType=function() return 0 end,
                    GetAssignedCity=function() return FOREIGN end,
                    IsEstablished=function() return true end,
                    GetNeutralizedTurns=function() return 0 end
                }
            end
        }
    end
end

function C.bad_definition(kind)
    lose()
    GameEffects.GetModifierDefinition=function()
        if kind=='nil' then return nil end
        if kind=='empty' then return {} end
        if kind=='false' then return {Id=false} end
        if kind=='blank' then return {Id=''} end
        error('unknown bad definition')
    end
end

function C.bad_transfer(kind)
    lose()
    local transfer=Game:GetProperty('JAC_BOUNDARY_CONFIG').transfer
    if kind=='city_id' then TARGET.id=77
    elseif kind=='plot' then TARGET.x=3
    elseif kind=='missing' then Game:GetProperty('JAC_BOUNDARY_CONFIG').transfer=nil
    elseif kind=='from_id' then transfer.fromCityID=77
    else error('unknown transfer mutation') end
end

function C.changed_transfer_result()
    local transfer=CityManager.TransferCityToFreeCities
    CityManager.TransferCityToFreeCities=function(city)
        transfer(city)
        city.id=77
    end
    fire(GameEvents.JAC_BoundaryLoseCity,0)
end

function C.gameplay_identity(field, value)
    lose()
    local read=ExposedMembers.JAC_PersonaBoundaryGameplayRead
    ExposedMembers.JAC_PersonaBoundaryGameplayRead=function()
        local result=read()
        result.cities['2/0'][field]=value
        return result
    end
end

function C.gameplay_only_city(marker)
    lose()
    local read=ExposedMembers.JAC_PersonaBoundaryGameplayRead
    ExposedMembers.JAC_PersonaBoundaryGameplayRead=function()
        local result=read()
        result.cities['9/9']={id=99,owner=0,settled=marker,selectors={}}
        return result
    end
end

function C.prepare_inputs(title_delta)
    Game:SetProperty('JAC_BOUNDARY_CONFIG',nil)
    WRITES={}
    APPOINTED=false
    PlayerConfigurations={
        [0]={GetLeaderTypeName=function() return 'LEADER_JAC_HOJO_UJITSUNA' end},
        [1]={GetLeaderTypeName=function() return LEADER end}}
    GameInfo.JAC_PersonaTitleCivics=collection({{CivicType='CIVIC_FIXTURE'}},'CivicType')
    GameInfo.Civics={CIVIC_FIXTURE={Index=7}}
    POINTS,COMPLETED,JOIN_CALLS=0,false,0
    Players[0].GetGovernors=function()
        return {
            GetGovernorPoints=function() return POINTS end,
            GetGovernorPointsSpent=function() return 0 end
        }
    end
    Players[0].GetCulture=function()
        return {
            HasCivic=function(_,index) assert(index==7); return COMPLETED end,
            GetCultureCost=function(_,index) assert(index==7); return 100 end,
            SetCulturalProgress=function(_,index,amount)
                assert(index==7 and amount==100)
                COMPLETED=true
                POINTS=POINTS+title_delta
            end
        }
    end
    ExposedMembers.JAC_LeaderPersona={Join=function(owner,source)
        assert(owner==0 and source==1 and POINTS==1)
        JOIN_CALLS=JOIN_CALLS+1
        return 'JOINED'
    end}
end
