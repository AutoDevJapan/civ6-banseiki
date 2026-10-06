-- Opt-in diagnostic headroom; not normal costs or economic balance.
UPDATE Units SET Cost=32768 WHERE UnitType IN ('UNIT_SPEARMAN','UNIT_BUILDER','UNIT_GALLEY');
UPDATE Buildings SET Cost=32768 WHERE BuildingType IN ('BUILDING_WALLS','BUILDING_CASTLE','BUILDING_STAR_FORT');
