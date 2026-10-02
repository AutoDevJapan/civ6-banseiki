
      assert(R.Turn()==t.FastLock,'wrong shortened unlock boundary')
      t.TrainingBefore=TrainingUnits(t.Actor)
      Players[t.Actor]:GetCities():FindID(t.Cities[2]):SetProperty(R.INPUT,{Building=10000})
      assert(R.StartProduction(t.Actor,t.Cities[2])); local s=R.State(t.Actor)
      t.MinStart=R.Turn(); assert(s.Production.Start==t.MinStart and s.Production.Turns==20 and s.Production.Due==t.MinStart+20,'minimum rule')
      assert(s.LastCompletion==t.Completion,'minimum completed early')
      assert(R.TestAdvanceDeadlines(t.Actor,{Training=true}))
      Game:SetProperty('JAC_ASN_LIVE_TEST',t); return 'nationwide unlock permits other city; normal minimum 20 verified before acceleration'
␠␠␠␠