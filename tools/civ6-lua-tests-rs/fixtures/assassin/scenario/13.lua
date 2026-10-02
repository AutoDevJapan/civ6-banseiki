
      local s=R.State(t.Actor); assert(Same(s,t.LockState),'lock/state lost')
      TrainedUnit(t.Actor,s.LastProducedUnit)
      assert(Same(TrainedSnapshot(t.Actor,s.LastProducedUnit),t.LockUnit),'load changed first parked individual')
      assert(Same(R.Nation(t.Target),t.FreeNation),'free-city history or people lost')
      assert(not Players[t.Target]:IsAlive(),'load resurrected retired slot')
      for _,c in ipairs(t.FreeAssets) do local actual=CityManager.GetCityAt(c.X,c.Y)
        assert(actual and actual:GetOwner()==t.FreeNation.FreeCitiesPlayer,'load lost free city') end
      local ok,why=R.CanProduce(s,t.Lock-1); assert(not ok and why=='PRODUCTION_LOCK','completion+9 boundary')
      assert(R.CanProduce(s,t.Lock),'completion+10 boundary')
      assert(R.TestAdvanceDeadlines(t.Actor,{ProductionLock=true,CommandLock=true}))
      t.FastLock=R.Turn()+1; Game:SetProperty('JAC_ASN_LIVE_TEST',t)
      for _,city in ipairs(t.Cities) do local ok,why=R.StartProduction(t.Actor,city); assert(not ok and why=='PRODUCTION_LOCK','shortened lock early') end
      return 'saved original +9/+10 boundaries verified; shortened nationwide locks still reject before deadline'
␠␠␠␠