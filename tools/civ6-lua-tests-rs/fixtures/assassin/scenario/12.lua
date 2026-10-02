
      local s=R.State(t.Actor); local n=R.Nation(t.Target)
      assert(R.Turn()==t.FastDue and s.Mission==nil and s.LastMission.Result=='LEADER_FREE_STATE','mission result')
      assert(n.State=='FREE_STATE' and n.LeaderDead==1 and n.DeathTurn==t.FastDue and n.Government=='FREE_CITIES')
      assert(n.Slot=='RETIRED' and n.AI=='FREE_CITIES_ENGINE' and not Players[t.Target]:IsAlive(),'free-city old slot')
      assert(Players[t.Target]:GetCities():GetCount()==0 and Players[t.Target]:GetUnits():GetCount()==0,'old assets remain')
      local free=PlayerManager.GetFreeCitiesPlayerID(); assert(n.FreeCitiesPlayer==free and Players[free]:GetCities():GetCount()>0,'free cities absent')
      for _,c in ipairs(t.FreeAssets) do local actual=CityManager.GetCityAt(c.X,c.Y)
        assert(actual and actual:GetOwner()==free,'city did not become free') end
      assert(n.People.JAC_LIVE_FREE_PERSON.Role=='UNAFFILIATED' and n.People.JAC_LIVE_FREE_PERSON.Unit==nil,'free person not archived')
      assert(n.People.JAC_LIVE_FREE_PERSON.SourceUnit==t.FreePersonUnit,'free person identity lost')
      local ok,why=R.CanCommand(s,R.Turn()); assert(not ok and why=='COMMAND_LOCK','command wait')
      assert(s.Production==nil and s.LastCompletion==t.FastDue and Players[t.Actor]:GetUnits():FindID(s.LastProducedUnit),'training not processed')
      NewTrainedUnit(t.Actor,t.TrainingBefore,s.LastProducedUnit)
      t.LockUnit=ParkTrainedUnit(t.Actor,s.LastProducedUnit,t.Cities[1])
      assert(s.ProductionUnlock==s.LastCompletion+10,'lock origin')
      for _,city in ipairs(t.Cities) do local ok,why=R.StartProduction(t.Actor,city); assert(not ok and why=='PRODUCTION_LOCK') end
      R.CancelProduction(t.Actor); assert(R.State(t.Actor).ProductionUnlock==s.ProductionUnlock,'cancel cleared lock')
      t.Completion=s.LastCompletion; t.Lock=s.ProductionUnlock; t.FreeNation=n; t.LockState=R.State(t.Actor)
      Game:SetProperty('JAC_ASN_LIVE_TEST',t); return 'real turn creates free cities and trained unit; nationwide 10-turn lock'
␠␠␠␠