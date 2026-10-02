
BASE_CALLS, ICONS = 0, {}
function Refresh() BASE_CALLS = BASE_CALLS + 1; Controls.Title:SetText("LOC_ERA_PROGRESS_TITLE") end
function PopulateCivilizationIcons()
  ICONS = {}
  local eras = Game.GetEras()
  for _, p in ipairs(PlayerManager.GetAliveMajors()) do
    local id = p:GetID()
    if id ~= Game.GetLocalPlayer() then
      local icon = "[ICON_GLORY_NORMAL_AGE]"
      if eras:HasHeroicGoldenAge(id) then icon = "[ICON_GLORY_SUPER_GOLDEN_AGE]"
      elseif eras:HasGoldenAge(id) then icon = "[ICON_GLORY_GOLDEN_AGE]"
      elseif eras:HasDarkAge(id) then icon = "[ICON_GLORY_DARK_AGE]" end
      ICONS[id] = icon
    end
  end
end
