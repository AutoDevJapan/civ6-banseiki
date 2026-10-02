' and n.DecisionRequired==nil,'branch unresolved')
          assert(n.DeathTurn==t.BranchDue and Same(n.Plan,t.BranchPlan) and Same(n.Snapshot,t.BranchSnapshot),'frozen plan changed')
␠␠␠␠␠␠␠␠