
      Game:SetProperty(R.TEST,1)
      local houses={}; for _,p in ipairs(PlayerManager.GetAliveMajors()) do
        houses[PlayerConfigurations[p:GetID()]:GetCivilizationTypeName()]=p:GetID()
      end
      local a,b=houses.CIVILIZATION_JAC_ODA,houses.CIVILIZATION_JAC_MATSUDAIRA
      assert(a and b,'test houses missing'); assert(not Players[b]:IsHuman(),'target must be AI')
      local city=Players[b]:GetCities():GetCapitalCity(); local u=Grant(a,R.UNIT,city)
      assert(PlayersVisibility[a]:IsRevealed(city:GetX(),city:GetY()),'fixture capital not revealed')
      local gp=Grant(b,'UNIT_GREAT_GENERAL',city)
      Players[b]:SetProperty(R.INPUT,{Successors={{Identity='JAC_LIVE_FREE_PERSON',Kind='GREAT_PERSON',Unit=gp:GetID()}}})
      local assets={}; for _,c in Players[b]:GetCities():Members() do assets[#assets+1]={X=c:GetX(),Y=c:GetY()} end
      local cities={}; for _,c in Players[a]:GetCities():Members() do cities[#cities+1]=c:GetID() end
      table.sort(cities); assert(#cities>=2,'two training cities required')
      Game:SetProperty(R.TEST,1); Players[a]:SetProperty(R.INPUT,{Security=50})
      Players[a]:GetCities():FindID(cities[1]):SetProperty(R.INPUT,nil)
      Players[a]:SetProperty('JAC_ASN_FORCE_NEXT',{Success=1,Detected=0,Branch='FREE_STATE'})
      Game:SetProperty('JAC_ASN_LIVE_TEST',{Actor=a,Target=b,Unit=u:GetID(),Cities=cities,Houses=houses,
        Start=R.Turn(),FreeAssets=assets,FreePersonUnit=gp:GetID(),LossTarget=houses.CIVILIZATION_JAC_IMAGAWA})
      assert(R.CHANCES.LEADER==1 and R.ProductionTurns({})==100 and R.ProductionTurns({Building=1000})==20)
      assert(R.WaitTurns({})==50 and R.WaitTurns({Security=0,Relation=1,School=1,Governor=1,Hero=1,GreatPerson=1})==1)
      return 'base 100/minimum 20; no-return unit grant found by ID delta'
␠␠␠␠