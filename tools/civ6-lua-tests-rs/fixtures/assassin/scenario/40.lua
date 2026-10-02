
      assert(Same(Game:GetProperty(R.NATIONS),t.FinalNations),'load changed national histories/people/plans')
      local ledger=Players[t.LossTarget]:GetProperty('JAC_ASN_DIPLOMACY_V1')
      assert(ledger.Grievances[tostring(t.Actor)]==t.FinalGrievances and ledger.Friendship[tostring(t.Actor)]==t.FinalFriendship,'load changed penalties')
      for id,applied in pairs(t.FinalApplied) do assert(ledger.Applied[id]==applied,'load lost applied mission') end
      for id,applied in pairs(ledger.Applied) do assert(t.FinalApplied[id]==applied,'load added applied mission') end
      local s=R.State(t.Actor); assert(Same(s,t.FinalState),'load changed state')
      TrainedUnit(t.Actor,t.LockState.LastProducedUnit); TrainedUnit(t.Actor,s.LastProducedUnit)
      for key,row in pairs(t.FinalUnits) do
        assert(Same(TrainedSnapshot(t.Actor,tonumber(key)),row),'load changed trained individual: '..key)
      end
      assert(s.LastMission.ID==t.FinalMissionID and s.LastMission.Result=='ACTOR_GONE' and s.CommandUnlock==t.FinalWait,'load changed result or wait')
      R.Tick(); return 'all branch histories, cancelled results and pre-save penalties survive load'
␠␠␠␠