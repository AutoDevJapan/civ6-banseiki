-- Diagnostic only. AI1 (Date) can natively consider only Ashina's Governor.
-- This static gate exists before Join, so it is NOT a product solution for
-- unjoined-person eligibility. No title is granted until the synchronized Join.
INSERT INTO Types (Type, Kind) VALUES
 ('TRAIT_JAC_TEST_ENEMY_CLOSED', 'KIND_TRAIT'),
 ('TRAIT_JAC_TEST_DATE_ASHINA', 'KIND_TRAIT');
INSERT INTO Traits (TraitType, Name, Description) VALUES
 ('TRAIT_JAC_TEST_ENEMY_CLOSED', 'LOC_JAC_PERSONA_CLASS', 'LOC_JAC_PERSONA_CLASS'),
 ('TRAIT_JAC_TEST_DATE_ASHINA', 'LOC_JAC_PERSONA_CLASS', 'LOC_JAC_PERSONA_CLASS');
UPDATE Governors SET TraitType='TRAIT_JAC_TEST_ENEMY_CLOSED'
 WHERE GovernorType IN (SELECT GovernorType FROM JAC_LeaderPersonas);
UPDATE Governors SET TraitType='TRAIT_JAC_TEST_DATE_ASHINA'
 WHERE GovernorType=(SELECT GovernorType FROM JAC_LeaderPersonas
                     WHERE LeaderType='LEADER_JAC_ASHINA_MORIKIYO');
INSERT INTO CivilizationTraits (CivilizationType, TraitType)
 VALUES ('CIVILIZATION_JAC_DATE', 'TRAIT_JAC_TEST_DATE_ASHINA');
