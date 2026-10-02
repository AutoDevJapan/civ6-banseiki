})
          FixtureReveal(t.Actor,victim:GetX(),victim:GetY())
          assert(R.StartAttempt(t.Actor,u:GetID(),b,'UNIT',victim:GetID()))
          local m=R.State(t.Actor).Mission; local ledger=Players[b]:GetProperty('JAC_ASN_DIPLOMACY_V1') or {}
          local distance=Map.GetPlotDistance(u:GetX(),u:GetY(),victim:GetX(),victim:GetY())
          assert(m.Due==R.Turn()+(distance<=1 and 2 or distance<=3 and 3 or 4),'unit mission distance rule')
          t.LossMission=m; t.LossVictim=victim:GetID(); t.LossGrievances=(ledger.Grievances or {})[tostring(t.Actor)] or 0
          t.LossCommandUnlock=R.State(t.Actor).CommandUnlock
          t.LossFriendship=(ledger.Friendship or {})[tostring(t.Actor)] or 0
          UnitManager.Kill(u); assert(R.TestAdvanceDeadlines(t.Actor,{Mission=true}))
          assert(R.State(t.Actor).CommandUnlock==t.LossCommandUnlock,'hook changed lost-actor wait')
          Game:SetProperty('JAC_ASN_LIVE_TEST',t)
          return 'lost-actor fixture with success=