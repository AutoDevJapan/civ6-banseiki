
      local watch=JAC_ASN_LIVE_WATCH
      local ok,why=pcall(function()
      assert(watch and watch.Owner==t.Actor,'unit watch missing')
      for key,row in pairs(watch.Rows) do
        local u=Players[watch.Owner]:GetUnits():FindID(tonumber(key))
        if tonumber(key)==t.ExpectedNativeDeath then
          assert(not u and row.Removed and row.Damage>=row.MaxDamage,'trained native damage/removal fixture not observed')
          print('[JAC_ASN_FIXTURE] trained native death confirmed id='..key)
        else
          assert(u and not row.Removed and u:GetType()==row.Type and R.IsAssassin(u) and
            u:GetX()==row.X and u:GetY()==row.Y and u:GetDamage()==row.InitialDamage and
            not row.DamageChanged and row.Damage==row.InitialDamage,
            'surviving trained individual changed after native fixture')
        end
      end
      end)
      if JAC_ASN_LIVE_DAMAGE then Events.UnitDamageChanged.Remove(JAC_ASN_LIVE_DAMAGE) end
      if JAC_ASN_LIVE_REMOVED then Events.UnitRemovedFromMap.Remove(JAC_ASN_LIVE_REMOVED) end
      JAC_ASN_LIVE_WATCH=nil; JAC_ASN_LIVE_DAMAGE=nil; JAC_ASN_LIVE_REMOVED=nil
      assert(ok,why)
      return 'first trained individual has witnessed native lethal damage and removal; second remains unchanged'
␠␠␠␠