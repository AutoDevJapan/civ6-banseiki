
          assert(n.State=='SUCCESSION' and n.CurrentLeader=='JAC_LIVE_HEIR' and n.Slot=='RETAINED')
          assert(Players[t.BranchTarget]:GetUnits():FindID(t.HeirUnit)==nil,'GP occupies two roles')
          assert(R.ResolveTarget(t.BranchTarget,'LEADER').Identity=='JAC_LIVE_HEIR','successor not targetable')
␠␠␠␠␠␠␠␠