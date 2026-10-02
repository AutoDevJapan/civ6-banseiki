
      local s=R.State(t.Actor); assert(Same(s.Production,t.Dynamic),'training lost on load')
      assert(s.Production.Start==t.Start and s.Production.Due==t.Start+120,'training clocks lost')
      assert(Same(s.Mission,t.Frozen),'mission snapshot/SuccessRoll/DetectionRoll/BranchRoll rerolled')
      assert(s.Mission.PostDeathPlan.Policy.FreeState=='ENGINE_FREE_CITIES','plan lost')
      assert(s.CommandUnlock==t.CommandUnlock,'command wait lost')
      Players[t.Actor]:SetProperty(R.INPUT,{Security=100,Relations={[tostring(t.LossTarget)]=1}})
      return 'load retains all training and mission fields; hostility set on a third country'
␠␠␠␠