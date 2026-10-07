-- The shared StagingRoom UI clamps AI slots as well as network peers to twelve.
-- A single-machine hotseat session can use the fixed Japan map's full roster.
local japanMaps = {
  ['{403be889-1455-4de7-967b-ad54c0ff3431}Maps/JAC_Japan_20km.Civ6Map'] = true,
  ['{403be889-1455-4de7-967b-ad54c0ff3431}Maps/JAC_Japan_20km_Settlers.Civ6Map'] = true,
  ['{403be889-1455-4de7-967b-ad54c0ff3431}Maps/JAC_Japan_15km.Civ6Map'] = true,
  ['{403be889-1455-4de7-967b-ad54c0ff3431}Maps/JAC_Japan_15km_Settlers.Civ6Map'] = true,
}

function JAC_JapanHotseatMaxPlayers(originalLimit)
  if GameConfiguration.IsHotseat()
    and GameConfiguration.GetValue('RULESET') == 'RULESET_JAC_BANSEIKI'
    and japanMaps[MapConfiguration.GetValue('MAP_SCRIPT')]
    and MapConfiguration.GetMaxMajorPlayers() == 22 then
    return 22
  end
  return originalLimit
end
