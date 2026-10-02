printed={}; function print(s) printed[#printed+1]=s end
state={turn=1, era=0}
local erasRows={[0]={EraType='ERA_ANCIENT'},[1]={EraType='ERA_CLASSICAL'}}
local function iter(t) local i=0; return function() i=i+1; return t[i] end end
GameInfo={Eras=erasRows, Technologies=function() return iter({{Index=0},{Index=1}}) end,
          Civics=function() return iter({{Index=0}}) end}
local eras={GetCurrentEra=function() return state.era end, GetCurrentEraStartTurn=function() return 1 end,
            GetNextEraCountdown=function() error('boom') end}
local p={GetID=function() return 0 end, GetEra=function() return state.era end,
         GetTechs=function() return {HasTech=function(_, i) return i==0 end} end,
         GetCulture=function() return {} end}
Game={GetCurrentGameTurn=function() return state.turn end, GetEras=function() return eras end}
GameConfiguration={GetStartTurn=function() return 1 end}
PlayerManager={GetAliveMajors=function() return {p} end}
