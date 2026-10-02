
      local s=R.State(t.Actor); assert(R.Turn()==t.Start+4,'turn count')
      assert(s.Production.Due==t.Start+70 and s.Production.Turns==70,'security and hostility did not shorten training')
      assert(s.Production and s.Mission and s.LastCompletion==nil,'premature result')
      assert(R.TestAdvanceDeadlines(t.Actor,{Training=true,Mission=true}))
      local accelerated=R.State(t.Actor); t.FastDue=R.Turn()+1; Game:SetProperty('JAC_ASN_LIVE_TEST',t)
      assert(accelerated.Production.Due==t.FastDue and accelerated.Mission.Due==t.FastDue,'test deadlines not saved')
      assert(accelerated.Production.Start==t.Start and accelerated.Production.Turns==70,'hook changed training rule')
      assert(accelerated.CommandUnlock==s.CommandUnlock and accelerated.Mission.SuccessRoll==s.Mission.SuccessRoll,'hook changed outcome/wait')
      return 'normal 70-turn deadline verified; test-only training and mission deadlines shortened'
␠␠␠␠