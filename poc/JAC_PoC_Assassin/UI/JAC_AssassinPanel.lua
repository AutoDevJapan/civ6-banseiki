-- Display and synchronized requests only. Gameplay and UI filenames must differ.
include "InstanceManager"
include "JAC_Assassin_Rules"
local R=JAC_ASN
local rows=InstanceManager:new("AssassinRow","Root",Controls.Rows)
local function Active()
  local p=Players[Game.GetLocalPlayer()]; return p and p:IsTurnActive()
end
local function Send(params)
  if not Active() then return false end
  params.OnStart="JAC_AssassinOperation"
  UI.RequestPlayerOperation(Game.GetLocalPlayer(),PlayerOperations.EXECUTE_SCRIPT,params)
  return true
end
function JAC_AssassinProduce(cityID) return Send({Action="PRODUCE",City=cityID}) end
function JAC_AssassinCancel() return Send({Action="CANCEL"}) end
function JAC_AssassinAttempt(unitID,target,kind,objectID)
  return Send({Action="ATTEMPT",Unit=unitID,Target=target,Kind=kind,Object=objectID})
end
local function Name(id)
  local nation=R.Nation(id)
  if nation and nation.CountryName then return Locale.Lookup(nation.CountryName) end
  return Locale.Lookup(PlayerConfigurations[id]:GetCivilizationDescription())
end
local function Add(text,caption,enabled,callback,reason)
  local row=rows:GetInstance(); row.Text:SetText(text); row.Action:SetText(Locale.Lookup(caption))
  row.Action:SetDisabled(not enabled or not Active()); row.Action:RegisterCallback(Mouse.eLClick,callback)
  row.Action:SetToolTipString(reason and Locale.Lookup("LOC_JAC_ASN_REASON_"..reason) or "")
end
local function Refresh()
  local id=Game.GetLocalPlayer(); local player=Players[id]
  Controls.OpenButton:SetHide(not player or not player:IsAlive())
  if not player then return end
  rows:ResetInstances()
  local s=R.State(id); local turn=R.Turn()
  Controls.Status:SetText(Locale.Lookup("LOC_JAC_ASN_STATUS",
    math.max(0,(s.ProductionUnlock or turn)-turn),math.max(0,(s.CommandUnlock or turn)-turn)))
  if s.Production then
    Add(Locale.Lookup("LOC_JAC_ASN_TRAINING",s.Production.City,math.max(0,s.Production.Due-turn)),
      "LOC_JAC_ASN_CANCEL",true,function() JAC_AssassinCancel() end)
  else
    local cities={}; for _,city in player:GetCities():Members() do cities[#cities+1]=city end
    table.sort(cities,function(a,b) return a:GetID()<b:GetID() end)
    for _,city in ipairs(cities) do
      local cityID=city:GetID(); local ok,reason=R.CanProduce(s,turn)
      Add(Locale.Lookup("LOC_JAC_ASN_CITY",Locale.Lookup(city:GetName()),R.ProductionTurns(R.ProductionInput(id,city))),
        "LOC_JAC_ASN_PRODUCE",ok,function() JAC_AssassinProduce(cityID) end,reason)
    end
  end
  if s.Mission then
    Add(Locale.Lookup("LOC_JAC_ASN_MISSION",Name(s.Mission.Target.Owner),math.max(0,s.Mission.Due-turn)),
      "LOC_JAC_ASN_WAIT",false,function() end)
  end
  -- One row per eligible nearby person/unit. A player chooses an actual map unit; no teleport command.
  local assassin=UI.GetHeadSelectedUnit()
  if assassin and assassin:GetOwner()==id and R.IsAssassin(assassin) then
    local targets={}; for _,p in ipairs(PlayerManager.GetAliveMajors()) do
      if p:GetID()~=id then targets[#targets+1]=p:GetID() end
    end; table.sort(targets)
    for _,target in ipairs(targets) do
      local function Candidate(kind,object,label)
        local dest=R.ResolveTarget(target,kind,object)
        if not dest then return end
        if not R.KnownTarget(id,dest) then return end
        local distance=Map.GetPlotDistance(assassin:GetX(),assassin:GetY(),dest.X,dest.Y)
        if distance>R.PROVISIONAL.MaximumDistance then return end
        local ok,reason=R.CheckAttempt(id,assassin:GetID(),target,kind,object)
        local missionTurns=R.MissionTurns(kind,distance)
        if not missionTurns then
          Add(Name(target).." / "..label,"LOC_JAC_ASN_ATTEMPT",false,function() end,"DURATION_UNSET")
          return
        end
        local input=R.Input(Players[target])
        local actual=R.SuccessChance(kind,{Security=input.Security,Relation=R.Relation(id,target),Guard=input.Guard})
        Add(Locale.Lookup("LOC_JAC_ASN_TARGET",Name(target),label,R.CHANCES[kind],missionTurns,actual),
          "LOC_JAC_ASN_ATTEMPT",ok,function() JAC_AssassinAttempt(assassin:GetID(),target,kind,object) end,reason)
      end
      local nation=R.Nation(target)
      Candidate("LEADER",nil,Locale.Lookup(nation and nation.CurrentLeaderName or PlayerConfigurations[target]:GetLeaderName()))
      local governors=R.Input(Players[target]).Governors or {}
      local keys={}; for key in pairs(governors) do keys[#keys+1]=key end; table.sort(keys)
      for _,key in ipairs(keys) do
        Candidate("GOVERNOR",tonumber(key),Locale.Lookup(governors[key].Name or "LOC_JAC_ASN_GOVERNOR"))
      end
      local units={}; for _,u in Players[target]:GetUnits():Members() do units[#units+1]=u end
      table.sort(units,function(a,b) return a:GetID()<b:GetID() end)
      for _,u in ipairs(units) do Candidate(R.Classify(u),u:GetID(),Locale.Lookup(u:GetName())) end
    end
  end
  if s.LastMission then Controls.Result:SetText(Locale.Lookup("LOC_JAC_ASN_RESULT",
    Locale.Lookup("LOC_JAC_ASN_RESULT_"..s.LastMission.Result))) end
  Controls.Rows:CalculateSize(); Controls.Scroll:CalculateInternalSize()
end
Controls.OpenButton:RegisterCallback(Mouse.eLClick,function()
  Controls.Panel:SetHide(not Controls.Panel:IsHidden()); Refresh()
end)
Controls.CloseButton:RegisterCallback(Mouse.eLClick,function() Controls.Panel:SetHide(true) end)
Events.LocalPlayerTurnBegin.Add(Refresh)
Events.UnitSelectionChanged.Add(Refresh)
Events.GameCoreEventPublishComplete.Add(Refresh)
Events.LoadScreenClose.Add(Refresh)
Refresh()
