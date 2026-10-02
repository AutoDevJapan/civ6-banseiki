
      local s=R.State(t.Actor)
      assert(s.Production.Due==t.Start+110 and s.Production.Turns==110,'hostility did not shorten training by 10')
      assert(Same(s.Mission,t.Frozen) and s.CommandUnlock==t.CommandUnlock,'relation changed frozen mission or wait')
      Players[t.Actor]:SetProperty(R.INPUT,{Security=100,Relations={[tostring(t.LossTarget)]=-1}})
      return 'worst living major hostility shortens training to 110; mission and command wait unchanged'
␠␠␠␠