
          local b=t.LossTarget; local city=Players[b]:GetCities():GetCapitalCity(); assert(city,'loss-test capital')
          local victim=Grant(b,'UNIT_WARRIOR',city); local u=Grant(t.Actor,R.UNIT,victim)
          local s=R.State(t.Actor); assert(s.Mission==nil and R.CanCommand(s,R.Turn()),'loss command wait')
          Players[t.Actor]:SetProperty('JAC_ASN_FORCE_NEXT',{Success=