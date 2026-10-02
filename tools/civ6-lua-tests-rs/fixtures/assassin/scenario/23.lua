
          local x=m.Snapshot.Selection; local best,score
          for _,n in ipairs(m.Snapshot.Neighbors) do
            assert(n.Strength==n.Cities+n.Military/x.MilitaryPerCity,'strength input')
            local f={Strength=n.Strength/math.max(1,x.MaxStrength),Friendship=n.Friendship,
              Border=n.Border and n.Border/math.max(1,x.MaxBorder),Distance=math.max(0,1-n.Distance/x.NeighborDistance),
              Religion=n.Religion,Culture=n.Culture}
            local sum,total=0,0
            for _,key in ipairs({'Strength','Friendship','Border','Distance','Religion','Culture'}) do
              if f[key]~=nil then sum=sum+x.Weights[key]*f[key]; total=total+x.Weights[key] end
            end
            local actual=sum/total; assert(math.abs(actual-n.Score)<0.000001,'receiver formula')
            if not score or actual>score or (actual==score and n.Player<best) then best,score=n.Player,actual end
          end
          assert(best and m.PostDeathPlan.Recipient==best,'receiver choice not maximum score')
␠␠␠␠␠␠␠␠