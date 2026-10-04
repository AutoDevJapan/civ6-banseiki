-- Explicit opt-in for the native archaeology acceptance scenario only.
-- Keep the official museum prerequisite, support capacity, cost and extraction.
-- Sengoku disables this unit; do not change the ordinary PoC production list.
UPDATE Units SET CanTrain = 1 WHERE UnitType = 'UNIT_ARCHAEOLOGIST';
