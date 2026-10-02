
          local s=R.State(t.Actor); local ledger=Players[t.LossTarget]:GetProperty('JAC_ASN_DIPLOMACY_V1')
          assert(s.LastMission.ID==t.LossMission.ID and s.LastMission.Result=='ACTOR_GONE' and s.LastMission.Cancelled,'ghost result')
          assert(Players[t.LossTarget]:GetUnits():FindID(t.LossVictim),'ghost success killed target')
          assert(ledger.Grievances[tostring(t.Actor)]==t.LossGrievances+25 and ledger.Friendship[tostring(t.Actor)]==t.LossFriendship-10,'fixed penalty')
          assert((ledger.WarReason or {})[tostring(t.Actor)]==nil,'ghost detection')
          for _,p in ipairs(PlayerManager.GetAliveMajors()) do assert(p:GetProperty('JAC_ASN_DENOUNCE_'..t.Actor)==nil,'ghost denunciation') end
          local before=ledger.Grievances[tostring(t.Actor)]; R.Tick()
          assert(Players[t.LossTarget]:GetProperty('JAC_ASN_DIPLOMACY_V1').Grievances[tostring(t.Actor)]==before,'duplicate penalty')
          assert(s.CommandUnlock==t.LossCommandUnlock,'wait changed')
          return 'all outcomes cancelled; fixed penalty once; original nationwide wait retained'
␠␠␠␠␠␠␠␠