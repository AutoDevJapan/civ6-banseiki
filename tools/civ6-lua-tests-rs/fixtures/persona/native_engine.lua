            PROPERTIES, EVER, NOW, APPOINTED, UNITS = {}, {}, {}, {}, {}
            Game={GetProperty=function(_,key) return PROPERTIES[key] end}
            ExposedMembers=setmetatable({}, {__index=function() error('persona bridge must not be consulted') end})
            Players, ROWS, DEFS, INDIVIDUALS = {}, {}, {}, {}
            for i=0,21 do
                local id=i
                local def={Index=i,Hash=9000+i,GovernorType='GOVERNOR_TEST_'..i}
                DEFS[def.GovernorType]=def
                ROWS[#ROWS+1]={LeaderType='LEADER_TEST_'..i,GovernorType=def.GovernorType,
                    PersonType='PERSON_TEST_'..i}
                INDIVIDUALS[2000+i]={GreatPersonIndividualType='PERSON_TEST_'..i}
                EVER[id],NOW[id],APPOINTED[id]={},{},{}
                UNITS[id]={}
                local function hash(value) return value<22 and value+9000 or value end
                local govs={
                    CanEverAppointGovernor=function(_,value) return EVER[id][hash(value)]==true end,
                    CanAppointGovernor=function(_,value) return NOW[id][hash(value)]==true end,
                    HasGovernor=function(_,value) return APPOINTED[id][value]==true end,
                    GetGovernor=function(_,value) return {GetType=function() return value-9000 end} end,
                    GetGovernorPoints=function() return id==0 and 0 or 1 end,
                    GetGovernorPointsSpent=function() return 0 end
                }
                Players[id]={GetID=function() return id end,
                    IsHuman=function() return id==0 end,GetGovernors=function() return govs end,
                    GetUnits=function() return {Members=function()
                        local nextUnit=0; return function()
                            nextUnit=nextUnit+1; local person=UNITS[id][nextUnit]
                            if person then return nextUnit,{GetType=function() return 5000 end,
                                GetGreatPerson=function() return {GetIndividual=function() return person end} end} end
                        end
                    end} end}
            end
            GameInfo={Governors=DEFS,GreatPersonClasses={GREAT_PERSON_CLASS_JAC_PERSONA={UnitType='UNIT_PERSONA'}},
                Units={UNIT_PERSONA={Index=5000}},GreatPersonIndividuals=INDIVIDUALS,JAC_LeaderPersonas=function()
                local i=0; return function() i=i+1; return ROWS[i] end
            end}
            PlayerManager={GetAliveMajors=function()
                local result={}; for i=0,21 do result[#result+1]=Players[i] end; return result
            end}

function index_disagreement(_, value)
    return value < 22
end
