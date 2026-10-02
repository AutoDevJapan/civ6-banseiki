'})
          assert(R.StartAttempt(t.Actor,u:GetID(),b,'LEADER')); local m=R.State(t.Actor).Mission
          local distance=Map.GetPlotDistance(u:GetX(),u:GetY(),city:GetX(),city:GetY())
          assert(m.Due==R.Turn()+(distance<=1 and 5 or distance<=3 and 8 or 10),'leader mission distance rule')
          t.BranchTarget=b; t.BranchPlan=m.PostDeathPlan; t.BranchSnapshot=m.Snapshot
          t.BranchAssets={}; for _,c in Players[b]:GetCities():Members() do t.BranchAssets[#t.BranchAssets+1]={X=c:GetX(),Y=c:GetY()} end
␠␠␠␠␠␠␠␠