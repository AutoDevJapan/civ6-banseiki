-- SSOT 3.2 resources. Runs after JAC_Sengoku_Trees.sql (the Sengoku techs exist) and BEFORE
-- JAC_Sengoku_Unlocks.sql, whose last statement deletes the official techs.
-- Resource_Harvests.PrereqTech is "ON DELETE SET NULL"
-- (Base/Assets/Gameplay/Data/Schema/01_GameplaySchema.sql:2360-2368), so a harvest whose official tech is
-- deleted would silently become available from turn 1. Re-point them to the Sengoku techs first.
-- (Resources.PrereqTech has the same SET NULL rule, 01_GameplaySchema.sql:2325-2343; JAC_Sengoku_Unlocks.sql
-- re-points it: iron, niter, horses and the late strategic resources.)
UPDATE Resource_Harvests SET PrereqTech='TECH_JAC_SENGOKU_IRRIGATION_WORKS'
WHERE ResourceType IN ('RESOURCE_RICE','RESOURCE_WHEAT','RESOURCE_BANANAS');
UPDATE Resource_Harvests SET PrereqTech='TECH_JAC_SENGOKU_ORE_PROSPECTING'
WHERE ResourceType IN ('RESOURCE_COPPER','RESOURCE_STONE');
UPDATE Resource_Harvests SET PrereqTech='TECH_JAC_SENGOKU_COASTAL_PILOTAGE'
WHERE ResourceType IN ('RESOURCE_FISH','RESOURCE_CRABS');
-- Cattle, deer and sheep (animal husbandry) do not exist on the Japan maps; they keep no tech gate.
