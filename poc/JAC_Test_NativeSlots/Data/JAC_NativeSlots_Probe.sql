-- PR86 diagnostic only; loaded by the separately enabled test MOD after the
-- title-proxy candidate. No persona identity, signature skill or payment queue.
-- XP1/XP2 Governors + GovernorPromotionModifiers; Base Modifiers.xml supplies
-- both property types. Normal governor activation before assignment is UNKNOWN.
INSERT INTO Types (Type, Kind) VALUES
 ('GOVERNOR_JAC_NATIVE_SLOT', 'KIND_GOVERNOR'),
 ('GOVERNOR_PROMOTION_JAC_NATIVE_SLOT_BASE', 'KIND_GOVERNOR_PROMOTION');
INSERT INTO Governors (GovernorType, Name, Description, Title, ShortTitle,
 IdentityPressure, TransitionStrength, AssignCityState, Image, PortraitImage, PortraitImageSelected)
VALUES ('GOVERNOR_JAC_NATIVE_SLOT', 'LOC_JAC_NATIVE_SLOT_NAME',
 'LOC_JAC_NATIVE_SLOT_DESCRIPTION', 'LOC_JAC_NATIVE_SLOT_NAME', 'LOC_JAC_NATIVE_SLOT_NAME',
 8, 100, 0, 'GOVERNOR_EDUCATOR', 'GovernorNormal_Educator', 'GovernorSelected_Educator');
-- Deliberately no GovernorsCannotAssign, TraitType or synthetic eligibility.
-- IdentityPressure=8 preserves ordinary loyalty and is an AI incentive under test.
-- TransitionStrength=100 with GOVERNOR_BASE_TURNS_TO_ESTABLISH=5 is ordinary timing.
INSERT INTO GovernorPromotions (GovernorPromotionType, Name, Description, Level, "Column", BaseAbility)
VALUES ('GOVERNOR_PROMOTION_JAC_NATIVE_SLOT_BASE', 'LOC_JAC_NATIVE_SLOT_BASE_NAME',
 'LOC_JAC_NATIVE_SLOT_BASE_DESCRIPTION', 0, 0, 1);
INSERT INTO GovernorPromotionSets (GovernorType, GovernorPromotion)
VALUES ('GOVERNOR_JAC_NATIVE_SLOT', 'GOVERNOR_PROMOTION_JAC_NATIVE_SLOT_BASE');
INSERT INTO Modifiers (ModifierId, ModifierType) VALUES
 ('JAC_NATIVE_SLOT_PLAYER_RECEIPT', 'MODIFIER_PLAYER_ADJUST_PROPERTY'),
 ('JAC_NATIVE_SLOT_CITY_ACTIVE', 'MODIFIER_SINGLE_CITY_ADJUST_PROPERTY');
INSERT INTO ModifierArguments (ModifierId, Name, Value) VALUES
 ('JAC_NATIVE_SLOT_PLAYER_RECEIPT', 'Key', 'JAC_NATIVE_SLOT_PLAYER_RECEIPT'),
 ('JAC_NATIVE_SLOT_PLAYER_RECEIPT', 'Amount', 1),
 ('JAC_NATIVE_SLOT_CITY_ACTIVE', 'Key', 'JAC_NATIVE_SLOT_CITY_ACTIVE'),
 ('JAC_NATIVE_SLOT_CITY_ACTIVE', 'Amount', 1);
INSERT INTO GovernorPromotionModifiers (GovernorPromotionType, ModifierId) VALUES
 ('GOVERNOR_PROMOTION_JAC_NATIVE_SLOT_BASE', 'JAC_NATIVE_SLOT_PLAYER_RECEIPT'),
 ('GOVERNOR_PROMOTION_JAC_NATIVE_SLOT_BASE', 'JAC_NATIVE_SLOT_CITY_ACTIVE');
-- No yield effect, GovernorModifiers, RunOnce/Permanent workaround or Lua writer
-- to either receipt. A city property is a diagnostic signal, not a settled proof
-- until it agrees with the actual native governor's IsEstablished().
