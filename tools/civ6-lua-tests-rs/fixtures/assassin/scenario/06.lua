
      local s=R.State(t.Actor); assert(s.Production.Turns==100 and s.Production.Speed==1,'base training')
      assert(s.Mission.Success and not s.Mission.Detected and s.Mission.Branch=='FREE_STATE','frozen outcome')
      assert(s.CommandUnlock-s.LastCommand==50,'command clock')
      local ok,why=R.StartProduction(t.Actor,t.Cities[2]); assert(not ok and why=='PRODUCING','parallel city')
      local u=Grant(t.Actor,R.UNIT,Players[t.Target]:GetCities():GetCapitalCity())
      local ok,why=R.StartAttempt(t.Actor,u:GetID(),t.Target,'LEADER'); assert(not ok and why=='MISSION_ACTIVE','second unit')
      t.SecondUnit=u:GetID(); t.Frozen=s.Mission; t.CommandUnlock=s.CommandUnlock
      t.TrainingBefore=TrainingUnits(t.Actor)
      Game:SetProperty('JAC_ASN_LIVE_TEST',t)
      Players[t.Actor]:SetProperty(R.INPUT,{Security=100}); return 'separate clocks and saved national plan'
␠␠␠␠