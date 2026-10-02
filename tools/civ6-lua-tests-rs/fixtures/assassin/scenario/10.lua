
      local s=R.State(t.Actor)
      assert(s.Production.Due==t.Start+120 and s.Production.Turns==120,'friendly relation did not restore training')
      Players[t.Actor]:SetProperty(R.INPUT,{Security=0,Relations={[tostring(t.LossTarget)]=1}})
      return 'friendly relations restore the 120-turn training deadline'
␠␠␠␠