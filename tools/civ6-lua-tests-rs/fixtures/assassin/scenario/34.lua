assert(R.TestAdvanceDeadlines(t.Actor,{CommandLock=true}))
              t.LossWait=R.Turn()+1; Game:SetProperty('JAC_ASN_LIVE_TEST',t)
              local ok,why=R.CanCommand(R.State(t.Actor),R.Turn()); assert(not ok and why=='COMMAND_LOCK')
              return 'next fixture waits one real turn for shortened nationwide command lock'