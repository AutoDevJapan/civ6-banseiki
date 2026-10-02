
      assert(Same(Game:GetProperty(R.NATIONS),t.FinalNations),'next turn changed national histories')
      local ledger=Players[t.LossTarget]:GetProperty('JAC_ASN_DIPLOMACY_V1')
      assert(ledger.Grievances[tostring(t.Actor)]==t.FinalGrievances and ledger.Friendship[tostring(t.Actor)]==t.FinalFriendship,'next turn repeated penalties')
      for id,applied in pairs(t.FinalApplied) do assert(ledger.Applied[id]==applied,'next turn lost applied mission') end
      for id,applied in pairs(ledger.Applied) do assert(t.FinalApplied[id]==applied,'next turn added applied mission') end
      local s=R.State(t.Actor); assert(Same(s,t.FinalState),'next turn changed state')
      assert(s.LastMission.ID==t.FinalMissionID and s.CommandUnlock==t.FinalWait,'next turn changed result or wait')
      assert(not Players[t.Target]:IsAlive() and not Players[t.Houses.CIVILIZATION_JAC_ASAKURA]:IsAlive(),'retired slot resurrected')
      assert(R.Turn()-t.Start<=20,'scenario exceeded live turn budget')
      Game:SetProperty(R.TEST,nil); Players[t.Actor]:SetProperty(R.INPUT,nil)
      local ok,why=R.TestAdvanceDeadlines(t.Actor); assert(not ok and why=='TEST_MODE_OFF','hook active outside test mode')
      assert(Same(R.State(t.Actor),s),'disabled hook changed saved state')
      return 'next turn cannot repeat effects; test mode disabled; hook inert; at most 20 end_turns'
␠␠␠␠