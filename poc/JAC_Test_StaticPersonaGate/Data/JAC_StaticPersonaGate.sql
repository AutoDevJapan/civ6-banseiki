-- Test-only copy of the live-34 valid trait schema. No house receives it.
INSERT INTO Types (Type, Kind) VALUES ('TRAIT_JAC_TEST_ALL_PERSONAS_CLOSED', 'KIND_TRAIT');
INSERT INTO Traits (TraitType, Name, Description) VALUES
 ('TRAIT_JAC_TEST_ALL_PERSONAS_CLOSED', 'LOC_JAC_PERSONA_CLASS', 'LOC_JAC_PERSONA_CLASS');
UPDATE Governors SET TraitType = 'TRAIT_JAC_TEST_ALL_PERSONAS_CLOSED'
 WHERE GovernorType IN (SELECT GovernorType FROM JAC_LeaderPersonas);
-- Intentionally no CivilizationTraits or LeaderTraits row for this trait.
