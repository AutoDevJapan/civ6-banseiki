local s=R.State(t.Actor); assert(R.Turn()==t.Start+1,'end_turn must honor turns')
      assert(s.Production.Due==t.Start+120 and s.Production.Turns==120,'current security did not extend training')
      t.Dynamic=s.Production; Game:SetProperty('JAC_ASN_LIVE_TEST',t)
      return 'training extended from 100 to 120 on the next turn'