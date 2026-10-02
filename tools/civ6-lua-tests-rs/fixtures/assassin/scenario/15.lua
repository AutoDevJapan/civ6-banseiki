local s=R.State(t.Actor); assert(s.LastCompletion==t.MinStart+1 and s.Production==nil,'shortened minimum completion')
      assert(s.ProductionUnlock==s.LastCompletion+10,'second nationwide lock')
      assert(Players[t.Actor]:GetUnits():FindID(s.LastProducedUnit),'second trained unit absent')
      assert(s.LastProducedUnit~=t.LockState.LastProducedUnit,'two trainings reused one unit')
      NewTrainedUnit(t.Actor,t.TrainingBefore,s.LastProducedUnit)
      ParkTrainedUnit(t.Actor,s.LastProducedUnit,t.Cities[2])
      Players[t.Actor]:GetCities():FindID(t.Cities[2]):SetProperty(R.INPUT,nil)
      return 'real turn completes second training and starts a new full 10-turn lock'