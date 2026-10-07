-- Fixed read-only InGame query. Session/barrier labels are added by the caller.
assert(GameConfiguration.IsLANMultiplayer(), 'LAN multiplayer required')
assert(GameConfiguration.IsNetworkMultiplayer(), 'network multiplayer required')
assert(not GameConfiguration.IsHotseat(), 'hotseat is not a LAN endpoint')
assert(Game:GetProperty('JAC_POC_GROWTHUI_INITIALIZED'), 'growth not initialized')
local turn = Game.GetCurrentGameTurn()
local localPlayer = Game.GetLocalPlayer()
assert(localPlayer == 0 or localPlayer == 1, 'local player must be 0 or 1')
local active = {}
local rows = {tostring(turn), tostring(localPlayer)}
local majors = PlayerManager.GetAliveMajors()
assert(#majors == 2, 'exactly two alive major players required')
local found = {}
for _, pid in ipairs(majors) do
  assert((pid == 0 or pid == 1) and not found[pid], 'unexpected major player')
  found[pid] = true
end
for pid = 0, 1 do
  local player = Players[pid]
  assert(found[pid] and player and player:IsHuman(), 'two human players required')
  active[pid] = player:IsTurnActive()
  local units = {}
  for _, unit in player:GetUnits():Members() do units[#units + 1] = unit end
  table.sort(units, function(a, b) return a:GetID() < b:GetID() end)
  local count = 0
  for _, unit in ipairs(units) do
    local record = unit:GetProperty('JAC_POC_GROWTHUI_WARLORD')
    if record then
      count = count + 1
      assert(type(record.Equipment) == 'table', 'invalid equipment')
      assert(type(record.Personality) == 'string' and type(record.Trait) == 'string', 'invalid growth metadata')
      local artifact = record.Artifact
      if artifact == nil then artifact = 'none' end
      local last = record.LastXPTurn
      if last == nil then last = -1 end
      rows[#rows + 1] = table.concat({pid, unit:GetID(), record.Level, record.XP,
        record.Personality, record.Trait, table.concat(record.Equipment, ','), artifact, last}, '~')
    end
  end
  assert(count == 2, 'expected two growth records per player')
end
assert(Game.GetCurrentGameTurn() == turn and Game.GetLocalPlayer() == localPlayer, 'collection crossed a turn boundary')
for pid = 0, 1 do
  assert(Players[pid]:IsTurnActive() == active[pid], 'collection crossed an activation boundary')
end
return table.concat(rows, '|')
