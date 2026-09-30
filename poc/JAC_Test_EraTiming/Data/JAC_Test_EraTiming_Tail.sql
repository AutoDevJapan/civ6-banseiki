-- Optional compressed 48-reign fixture, only on the unification test map with JAC_ERA_TIMING_TAIL=1.
-- Preserve the ordinary timing probe (3/5/1/2/2-4) when the flag is 0.
-- ChronologyIndex: Base/Assets/Gameplay/Data/Eras.xml:16; nullable lengths:
-- DLC/Expansion2/Data/Expansion2_Schema.sql:293-299. Engine minimum 2 is verified in era-equals-reign.md.
UPDATE Eras_XP1 SET GameEraMinimumTurns=2, GameEraMaximumTurns=2
WHERE EraType IN (SELECT EraType FROM Eras WHERE ChronologyIndex < 48);
UPDATE Eras_XP1 SET GameEraMinimumTurns=NULL, GameEraMaximumTurns=NULL WHERE EraType='ERA_JAC_CHIDAI';
INSERT OR REPLACE INTO GlobalParameters (Name, Value) VALUES ('JAC_TEST_ERA_TAIL_VERSION', '1');
