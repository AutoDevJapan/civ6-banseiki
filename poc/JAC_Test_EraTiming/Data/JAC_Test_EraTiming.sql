-- Era-timing probe (documents/poc/era-equals-reign.md). Official values: 40/60 for every era but
-- ERA_FUTURE (NULL) (DLC/Expansion2/Data/Expansion1_Eras.xml:3-13), countdown 10
-- (DLC/Expansion2/Data/Expansion2_GlobalParameters.xml:204). Distinct lengths so each change is identifiable;
-- ERA_INDUSTRIAL min < max shows whether anything ends an era before its maximum.
UPDATE Eras_XP1 SET GameEraMinimumTurns = 3, GameEraMaximumTurns = 3 WHERE EraType = 'ERA_ANCIENT';
UPDATE Eras_XP1 SET GameEraMinimumTurns = 5, GameEraMaximumTurns = 5 WHERE EraType = 'ERA_CLASSICAL';
UPDATE Eras_XP1 SET GameEraMinimumTurns = 1, GameEraMaximumTurns = 1 WHERE EraType = 'ERA_MEDIEVAL';
UPDATE Eras_XP1 SET GameEraMinimumTurns = 2, GameEraMaximumTurns = 2 WHERE EraType = 'ERA_RENAISSANCE';
UPDATE Eras_XP1 SET GameEraMinimumTurns = 2, GameEraMaximumTurns = 4 WHERE EraType = 'ERA_INDUSTRIAL';
INSERT OR REPLACE INTO GlobalParameters (Name, Value) VALUES ('NEXT_ERA_TURN_COUNTDOWN', '1');
