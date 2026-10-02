
      assert(Game:GetProperty(R.TEST)==1 and Game.GetLocalPlayer()==t.Actor,'unit watch requires the test human')
      local watch={Owner=t.Actor,Rows={}}
      for _,id in ipairs({t.LockState.LastProducedUnit,R.State(t.Actor).LastProducedUnit}) do
        local u=TrainedUnit(t.Actor,id)
        watch.Rows[tostring(id)]={Type=u:GetType(),X=u:GetX(),Y=u:GetY(),MaxDamage=u:GetMaxDamage(),
          InitialDamage=u:GetDamage(),Damage=u:GetDamage(),DamageChanged=false,Removed=false}
      end
      JAC_ASN_LIVE_WATCH=watch
      JAC_ASN_LIVE_DAMAGE=function(owner,id,damage)
        local row=owner==watch.Owner and watch.Rows[tostring(id)]
        if row then
          row.Damage=damage; row.DamageChanged=row.DamageChanged or damage~=row.InitialDamage
          print('[JAC_ASN_FIXTURE] trained damage id='..id..' damage='..damage)
        end
      end
      JAC_ASN_LIVE_REMOVED=function(owner,id)
        local row=owner==watch.Owner and watch.Rows[tostring(id)]
        if row then row.Removed=true; print('[JAC_ASN_FIXTURE] trained removed id='..id) end
      end
      Events.UnitDamageChanged.Add(JAC_ASN_LIVE_DAMAGE)
      Events.UnitRemovedFromMap.Add(JAC_ASN_LIVE_REMOVED)
      return 'both trained individuals survive load; native damage/removal watch installed for next turn'
␠␠␠␠