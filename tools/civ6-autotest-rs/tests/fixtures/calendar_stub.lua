function print() end
state={}
local rows={}; for i=1,99 do rows[i]={} end
Game={GetCurrentGameTurn=function() return state.turn end}
GameConfiguration={GetStartTurn=function() return 1 end,GetGameSpeedType=function() return 0 end}
GameInfo={GameSpeeds={[0]={GameSpeedType='GAMESPEED_EPIC',GameSpeedTurnCollection=rows}}}
ExposedMembers={JAC_Resources={CurrentYear=function() return state.year end}}
Calendar={MakeYearStr=function() return tostring(state.year)..' AD' end}
