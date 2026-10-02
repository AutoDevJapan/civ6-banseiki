
      local ledger=Players[t.LossTarget]:GetProperty('JAC_ASN_DIPLOMACY_V1')
      local s=R.State(t.Actor); assert(s.LastMission.Result=='ACTOR_GONE')
      t.FinalGrievances=ledger.Grievances[tostring(t.Actor)]; t.FinalFriendship=ledger.Friendship[tostring(t.Actor)]
      t.FinalApplied=ledger.Applied; t.FinalMissionID=s.LastMission.ID; t.FinalWait=s.CommandUnlock
      t.FinalNations=Game:GetProperty(R.NATIONS); t.FinalState=s
      t.FinalUnits={}
      for _,id in ipairs({t.LockState.LastProducedUnit,s.LastProducedUnit}) do
        t.FinalUnits[tostring(id)]=TrainedSnapshot(t.Actor,id); TrainedUnit(t.Actor,id)
      end
      Game:SetProperty('JAC_ASN_LIVE_TEST',t); return 'full national histories, state, penalties and applied missions recorded before save'
␠␠␠␠