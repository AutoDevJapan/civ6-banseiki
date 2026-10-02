; assert(b and Players[b]:IsAlive() and not Players[b]:IsHuman(),'branch target')
          local city=Players[b]:GetCities():GetCapitalCity(); assert(city,'capital absent')
          local u=Grant(t.Actor,R.UNIT,city); local input=R.Input(Players[b])
␠␠␠␠␠␠␠␠