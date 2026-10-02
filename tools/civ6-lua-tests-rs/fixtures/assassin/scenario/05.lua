assert(Game.GetLocalPlayer()==t.Actor,'wrong human')
      assert(JAC_AssassinProduce(t.Cities[1]),'training not sent')
      assert(JAC_AssassinAttempt(t.Unit,t.Target,'LEADER'),'attempt not sent')
      return 'training and mission sent via EXECUTE_SCRIPT during the active turn'