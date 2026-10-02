local function FixtureReveal(owner,x,y)
      local ok,why=R.TestRevealTile(owner,x,y)
      assert(ok,'fixture reveal failed: '..tostring(why))
    end; local function Grant(owner,kind,city)
      assert(Game:GetProperty(R.TEST)==1,'fixture grant requires test mode')
      local x,y=city:GetX(),city:GetY()
      assert(x>=0 and y>=0,'fixture anchor has invalid coordinates: '..x..','..y)
      local found
      for radius=1,6 do
        local before={}; for _,u in Players[owner]:GetUnits():Members() do before[u:GetID()]=true end
        UnitManager.InitUnitValidAdjacentHex(owner,kind,x,y,radius)
        for _,u in Players[owner]:GetUnits():Members() do
          if not before[u:GetID()] and u:GetType()==GameInfo.Units[kind].Index then
            if u:GetX()>=0 and u:GetY()>=0 and Map.GetPlotDistance(x,y,u:GetX(),u:GetY())<=6 then found=u
            else
              print('[JAC_ASN_FIXTURE] unplaced unit owner='..owner..' kind='..kind..' radius='..radius..
                ' xy='..u:GetX()..','..u:GetY())
              UnitManager.Kill(u)
            end
          end
        end
        if found then break end
      end
      assert(found,'fixture unit has no valid placement within six tiles: '..kind..' owner='..owner)
      if R.IsAssassin(found) then FixtureReveal(owner,x,y); FixtureReveal(owner,found:GetX(),found:GetY()) end
      return found
    end; local function TrainedUnit(owner,id)
      local u=Players[owner]:GetUnits():FindID(id)
      assert(u and R.IsAssassin(u) and u:GetX()>=0 and u:GetY()>=0,
        'trained unit is not placed on the map: id='..tostring(id)..' turn='..R.Turn()..
        ' type='..tostring(u and u:GetType())..' x='..tostring(u and u:GetX())..' y='..tostring(u and u:GetY()))
      return u
    end; local function TrainedSnapshot(owner,id)
      local u=Players[owner]:GetUnits():FindID(id)
      local row={ID=id,Present=u~=nil}
      if u then row.Type=u:GetType(); row.X=u:GetX(); row.Y=u:GetY(); row.Damage=u:GetDamage() end
      print('[JAC_ASN_TRACE] snapshot turn='..R.Turn()..' id='..id..' present='..tostring(row.Present)..
        ' type='..tostring(row.Type)..' xy='..tostring(row.X)..','..tostring(row.Y)..' damage='..tostring(row.Damage))
      return row
    end; local function ParkTrainedUnit(owner,id,cityID)
      assert(Game:GetProperty(R.TEST)==1,'trained parking requires test mode')
      local u=TrainedUnit(owner,id); local city=Players[owner]:GetCities():FindID(cityID)
      assert(city and city:GetOwner()==owner,'trained parking requires an owned city')
      local blockers={}
      for _,other in Players[owner]:GetUnits():Members() do
        if other:GetID()~=id and other:GetX()==city:GetX() and other:GetY()==city:GetY() then
          assert(not R.IsAssassin(other),'trained parking would remove another assassin')
          blockers[#blockers+1]=other
        end
      end
      -- Clear the fixture's native garrison to avoid stacking two combat units.
      -- Keep the production ID; do not replace it with a fixture-granted unit.
      for _,other in ipairs(blockers) do UnitManager.Kill(other) end
      -- Official DLC/AustraliaScenario/Scripts/AustraliaScenario.lua:854.
      UnitManager.PlaceUnit(u,city:GetX(),city:GetY())
      assert(u:GetX()==city:GetX() and u:GetY()==city:GetY(),'trained parking did not reach city center')
      return TrainedSnapshot(owner,id)
    end; local function TrainingUnits(owner)
      local ids={}; for _,u in Players[owner]:GetUnits():Members() do
        if R.IsAssassin(u) then ids[tostring(u:GetID())]=true end
      end; return ids
    end; local function NewTrainedUnit(owner,before,id)
      local count=0; for key in pairs(TrainingUnits(owner)) do if not before[key] then
        count=count+1; assert(tonumber(key)==id,'completion recorded an old or different unit')
      end end
      assert(count==1,'training did not create exactly one new assassin')
      return TrainedUnit(owner,id)
    end;␠