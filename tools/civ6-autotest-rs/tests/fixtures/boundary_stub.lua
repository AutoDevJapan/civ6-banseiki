function print() end
state={}; ledger={history={}}
local eras={GetCurrentEra=function() return state.era end,
            GetCurrentEraStartTurn=function() return state.start end}
Game={GetCurrentGameTurn=function() return state.turn end,
      GetEras=function() return eras end,
      GetProperty=function() return ledger end}
GameConfiguration={GetStartTurn=function() return 1 end}
GameInfo={Eras={}}
