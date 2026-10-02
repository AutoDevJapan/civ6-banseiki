
          Players[b]:SetProperty(R.INPUT,input)
          local s=R.State(t.Actor); assert(s.Mission==nil and R.CanCommand(s,R.Turn()),'branch wait not expired')
          Players[t.Actor]:SetProperty('JAC_ASN_FORCE_NEXT',{Success=1,Detected=0,Branch='