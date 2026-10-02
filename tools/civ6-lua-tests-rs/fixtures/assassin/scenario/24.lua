
          local plan=m.PostDeathPlan; local ballot=plan.Ballots[tostring(t.Actor)]
          assert(ballot.Favor==100000 and ballot.Weight>plan.Tally.FREE_STATE,'favor did not outweigh majority')
          assert(plan.Tally.SUCCESSION==ballot.Weight and plan.Resolution=='SUCCESSION' and plan.Voting=='FAVOR_WEIGHTED_EQUIVALENT','favor ballot')
          Players[t.Actor]:GetDiplomacy():ChangeFavor(-100000)
          input.Congress.Votes={}; Players[b]:SetProperty(R.INPUT,input)
␠␠␠␠␠␠␠␠