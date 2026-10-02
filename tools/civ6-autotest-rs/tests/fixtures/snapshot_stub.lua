state={population=3,owner=0,unitX=2,damage=4,unitPresent=true,initialized=true,
       warlord={Level=1,XP=20},ledger={reign=0,history={}}}
local city={GetID=function() return 7 end,GetX=function() return 2 end,
            GetY=function() return 3 end,GetPopulation=function() return state.population end}
local cities={Members=function() local done=false; return function()
    if done then return nil end; done=true; return 0,city
end end}
local research={HasTech=function() return true end}
local culture={HasCivic=function() return true end}
local unit={GetID=function() return 8 end,GetOwner=function() return 0 end,
            GetUnitType=function() return 1 end,GetX=function() return state.unitX end,
            GetY=function() return 3 end,GetDamage=function() return state.damage end,
            GetProperty=function(_,key) return state.warlord end}
local units={Members=function() local done=false; return function()
    if done or not state.unitPresent then return nil end; done=true; return 0,unit
end end}
local player={IsAlive=function() return true end,GetCities=function() return cities end,
              GetUnits=function() return units end,
              GetTechs=function() return research end,GetCulture=function() return culture end}
Players={};for i=0,21 do Players[i]=player end
Game={GetCurrentGameTurn=function() return 1 end,
      GetLocalPlayer=function() return 0 end,
      GetEras=function() return {GetCurrentEra=function() return 0 end} end,
      GetProperty=function(_,key) if key=='JAC_AGES_LEDGER' then return state.ledger
        elseif key=='JAC_POC_GROWTHUI_INITIALIZED' then return state.initialized end end}
local function rows(row) return function() local done=false; return function()
    if done then return nil end; done=true;return row
end end end
GameInfo={Technologies=rows({Index=0,TechnologyType='TECH_TEST'}),
          Civics=rows({Index=0,CivicType='CIVIC_TEST'})}
Map={GetPlotCount=function() return 2 end,
     GetPlotByIndex=function(i) return {GetOwner=function() return i==0 and 0 or state.owner end} end}
