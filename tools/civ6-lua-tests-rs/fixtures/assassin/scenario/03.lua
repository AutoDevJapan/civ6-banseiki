if t and t.LockState then
          TrainedSnapshot(t.Actor,t.LockState.LastProducedUnit)
          local id=R.State(t.Actor).LastProducedUnit
          if id and id~=t.LockState.LastProducedUnit then TrainedSnapshot(t.Actor,id) end
        end;␠