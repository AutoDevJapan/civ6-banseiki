-- One candidate only. The production 22-person closed gate remains intact.
-- Base Modifiers.xml declares a grant-trait modifier type but supplies no
-- DynamicModifiers row or example. EFFECT_GRANT_TRAIT is an unproved handler
-- candidate; the live scenario must observe native CanEver and APPOINT.
INSERT INTO Types (Type, Kind) VALUES
 ('TRAIT_JAC_TEST_ASHINA_JOINED', 'KIND_TRAIT'),
 ('MODIFIER_JAC_TEST_OWNER_GRANT_TRAIT', 'KIND_MODIFIER');
INSERT OR IGNORE INTO Types (Type, Kind)
 VALUES ('EFFECT_GRANT_TRAIT', 'KIND_EFFECT');
INSERT OR IGNORE INTO Types (Type, Kind)
 VALUES ('COLLECTION_OWNER', 'KIND_COLLECTION');
INSERT INTO Traits (TraitType, Name, Description)
 VALUES ('TRAIT_JAC_TEST_ASHINA_JOINED', 'LOC_JAC_PERSONA_CLASS', 'LOC_JAC_PERSONA_CLASS');
UPDATE Governors SET TraitType='TRAIT_JAC_TEST_ASHINA_JOINED'
 WHERE GovernorType=(SELECT GovernorType FROM JAC_LeaderPersonas
                     WHERE LeaderType='LEADER_JAC_ASHINA_MORIKIYO');
INSERT INTO DynamicModifiers (ModifierType, CollectionType, EffectType)
 VALUES ('MODIFIER_JAC_TEST_OWNER_GRANT_TRAIT', 'COLLECTION_OWNER', 'EFFECT_GRANT_TRAIT');
INSERT INTO Modifiers (ModifierId, ModifierType)
 VALUES ('JAC_TEST_ASHINA_JOINED_GRANT', 'MODIFIER_JAC_TEST_OWNER_GRANT_TRAIT');
INSERT INTO ModifierArguments (ModifierId, Name, Value)
 VALUES ('JAC_TEST_ASHINA_JOINED_GRANT', 'TraitType', 'TRAIT_JAC_TEST_ASHINA_JOINED');
-- No LeaderTraits, CivilizationTraits, GameModifiers or other global grant.
