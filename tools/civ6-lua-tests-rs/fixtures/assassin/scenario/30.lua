
          assert(n.State=='ABSORPTION' and n.Recipient==t.BranchPlan.Recipient,'wrong recipient')
          assert(Players[t.BranchTarget]:GetCities():GetCount()==0 and Players[t.BranchTarget]:GetUnits():GetCount()==0,'old assets remain')
          assert(not Players[t.BranchTarget]:IsAlive(),'empty native slot still active')
          for _,c in ipairs(t.BranchAssets) do local actual=CityManager.GetCityAt(c.X,c.Y)
            assert(actual and actual:GetOwner()==n.Recipient,'city transfer failed') end
          assert(Players[n.Recipient]:GetProperty(R.PEOPLE).JAC_LIVE_ABSORBED_PERSON.Role=='MEMBER','living person not joined')
          assert(n.Slot=='RETIRED' and n.AI=='RECIPIENT_SLOT','old AI retained')
␠␠␠␠␠␠␠␠