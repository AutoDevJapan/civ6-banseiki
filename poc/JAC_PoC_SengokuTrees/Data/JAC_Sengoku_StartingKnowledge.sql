-- The base game grants ERA_ANCIENT's StartingCivics before the first turn.
-- HOUSE_RULES replaces CODE_OF_LAWS in JAC_Sengoku_Unlocks.sql.
-- These two additional roots complete the shared provisional Era I civic set.
INSERT INTO StartingCivics (Era, Civic) VALUES
  ('ERA_ANCIENT', 'CIVIC_JAC_SENGOKU_KOKUJIN_LEAGUES'),
  ('ERA_ANCIENT', 'CIVIC_JAC_SENGOKU_TEMPLES_AND_BELIEVERS');
