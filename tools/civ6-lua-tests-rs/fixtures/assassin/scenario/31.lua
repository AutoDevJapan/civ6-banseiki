
          assert(n.State=='SUCCESSION' and n.Plan.Resolution=='SUCCESSION' and n.CurrentLeader=='JAC_LIVE_VOTE_HEIR','favor/ballot changed')
          assert(n.Plan.Tally.SUCCESSION>n.Plan.Tally.FREE_STATE and Players[t.BranchTarget]:IsAlive(),'favor-weighted vote failed')
          for id,favor in pairs(t.FavorBefore) do local diplo=Players[tonumber(id)]:GetDiplomacy()
            diplo:ChangeFavor(favor-diplo:GetFavor()) end
␠␠␠␠␠␠␠␠