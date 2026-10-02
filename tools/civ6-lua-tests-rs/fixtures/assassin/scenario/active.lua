local p=Players[Game.GetLocalPlayer()]
        if not p or not p:IsTurnActive() then return false end
        if not JAC_ASN_TRACE then
          JAC_ASN_TRACE=true
          Events.UnitDamageChanged.Add(function(owner,id,damage)
            local t=Game:GetProperty('JAC_ASN_LIVE_TEST')
            if t and owner==t.Actor then print('[JAC_ASN_TRACE] damage turn='..Game.GetCurrentGameTurn()..' id='..id..' damage='..damage) end
          end)
          Events.UnitRemovedFromMap.Add(function(owner,id)
            local t=Game:GetProperty('JAC_ASN_LIVE_TEST')
            if t and owner==t.Actor then print('[JAC_ASN_TRACE] removed turn='..Game.GetCurrentGameTurn()..' id='..id) end
          end)
        end
        return true