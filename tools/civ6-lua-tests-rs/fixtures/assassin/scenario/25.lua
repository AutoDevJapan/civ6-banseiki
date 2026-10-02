
          assert(R.TestAdvanceDeadlines(t.Actor,{Mission=true,CommandLock=true})); t.BranchDue=R.Turn()+1
          assert(Same(R.State(t.Actor).Mission.PostDeathPlan,t.BranchPlan),'hook changed plan')
          Game:SetProperty('JAC_ASN_LIVE_TEST',t); return '