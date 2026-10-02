
      assert(Game:GetProperty(R.TEST)==1,'native removal fixture requires test mode')
      local id=t.LockState.LastProducedUnit; local u=TrainedUnit(t.Actor,id)
      u:ChangeDamage(u:GetMaxDamage()-u:GetDamage())
      local remaining=Players[t.Actor]:GetUnits():FindID(id)
      if remaining then UnitManager.Kill(remaining) end
      assert(not Players[t.Actor]:GetUnits():FindID(id),'native removal fixture did not remove the individual')
      t.ExpectedNativeDeath=id; Game:SetProperty('JAC_ASN_LIVE_TEST',t)
      return 'fixture applies native lethal damage and removal to the first trained individual after save/load'
␠␠␠␠