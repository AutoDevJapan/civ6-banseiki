-- Opt-in spy diagnostic only. Sengoku intentionally hides these buildings.
-- Restore their official production visibility without changing costs,
-- requirements, effects, spy capacity or the base Sengoku building catalog.
UPDATE Buildings SET InternalOnly=0 WHERE BuildingType IN (
  'BUILDING_GOV_TALL', 'BUILDING_GOV_WIDE', 'BUILDING_GOV_CONQUEST',
  'BUILDING_GOV_SPIES'
);
