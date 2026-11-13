-- Independent comparison only: make the payment proxy unavailable to every
-- civilization, so its AI appointment cannot hide the ordinary slot's choice.
INSERT INTO Types (Type, Kind) VALUES ('TRAIT_JAC_TEST_PROXY_DISABLED', 'KIND_TRAIT');
INSERT INTO Traits (TraitType, Name, Description) VALUES
 ('TRAIT_JAC_TEST_PROXY_DISABLED', 'LOC_JAC_PERSONA_TITLE_PAYMENT_NAME', 'LOC_JAC_PERSONA_TITLE_PAYMENT_DESCRIPTION');
UPDATE Governors SET TraitType = 'TRAIT_JAC_TEST_PROXY_DISABLED'
 WHERE GovernorType = 'GOVERNOR_JAC_TITLE_PROXY';
-- Deliberately no CivilizationTraits or LeaderTraits grant this test trait.
-- The normal anonymous slot retains its definition, loyalty and eligibility.
