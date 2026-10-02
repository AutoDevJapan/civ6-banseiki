
          local votes={}; t.FavorBefore={}; local count=0
          for _,p in ipairs(PlayerManager.GetAliveMajors()) do
            local id=p:GetID(); local diplo=p:GetDiplomacy(); local favor=diplo:GetFavor()
            t.FavorBefore[tostring(id)]=favor; diplo:ChangeFavor(-favor)
            assert(diplo:GetFavor()==0,'favor reset'); votes[tostring(id)]='FREE_STATE'; count=count+1
          end
          assert(count>2,'favor fixture requires opposing majority')
          Players[t.Actor]:GetDiplomacy():ChangeFavor(100000); votes[tostring(t.Actor)]='SUCCESSION'
          input.Successor={Identity='JAC_LIVE_VOTE_HEIR',Kind='PERSON'}
          input.Congress={Enabled=true,Votes=votes}
␠␠␠␠␠␠␠␠