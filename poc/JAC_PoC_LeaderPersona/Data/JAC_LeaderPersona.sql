-- SSOT 3.2, 4.7, 9.4: owner decisions 2026-10-01 17:48 / 18:00 JST.
-- Provisional balance numbers; sources and official file:line evidence: documents/poc/leader-persona.md.
-- LoadOrder 400 AFTER SengokuTrees. Only the persona civic grants below restore titles.
CREATE TABLE JAC_PersonaCostParameters (Name TEXT PRIMARY KEY, Value INTEGER NOT NULL);
INSERT INTO JAC_PersonaCostParameters VALUES
    ('BASE_TITLES', 1),
    ('FAME_TITLE_WEIGHT', 1),
    ('YOUTH_AGE_LIMIT', 16),
    ('YOUTH_TITLE_SURCHARGE', 1),
    ('REPRESENTATIVE_TITLE_SURCHARGE', 1),
    ('MIN_TITLES', 1),
    ('MAX_TITLES', 3);

CREATE TABLE JAC_LeaderPersonas (
    LeaderType TEXT PRIMARY KEY REFERENCES Leaders(LeaderType),
    CivilizationType TEXT NOT NULL UNIQUE REFERENCES Civilizations(CivilizationType),
    PersonType TEXT NOT NULL UNIQUE,
    GovernorType TEXT NOT NULL UNIQUE,
    Name TEXT NOT NULL, PortraitIcon TEXT NOT NULL,
    BasePromotionType TEXT NOT NULL UNIQUE,
    Personality TEXT NOT NULL CHECK (Personality IN ('BRAVE','CALM','CAREFUL','JOLLY','GENTLE','BOLD','FICKLE','LOYAL')),
    Trait TEXT NOT NULL CHECK (Trait IN ('VALIANT','FIERCE','ASTUTE','SHREWD','DEVOUT','ASCETIC','MUNDANE','ADEPT')),
    Age1534 INTEGER CHECK (Age1534 >= 0),
    FameTier INTEGER NOT NULL CHECK (FameTier BETWEEN 0 AND 2),
    Role1534 TEXT NOT NULL CHECK (Role1534 IN ('HEAD','SHOGUN','RELIGIOUS_HEAD','KING','REPRESENTATIVE')),
    EvolutionCost INTEGER NOT NULL DEFAULT 1 CHECK (EvolutionCost >= 1)
);
CREATE TABLE JAC_PersonaDesigns (
    LeaderType TEXT PRIMARY KEY REFERENCES Leaders(LeaderType),
    Age1534 INTEGER, FameTier INTEGER NOT NULL, Role1534 TEXT NOT NULL,
    Personality TEXT NOT NULL, Trait TEXT NOT NULL
);
INSERT INTO JAC_PersonaDesigns VALUES
    ('LEADER_JAC_DATE_TANEMUNE', 46, 1, 'HEAD', 'CALM', 'ASTUTE'),
    ('LEADER_JAC_ASHINA_MORIKIYO', 44, 0, 'HEAD', 'CAREFUL', 'MUNDANE'),
    ('LEADER_JAC_HOJO_UJITSUNA', 47, 1, 'HEAD', 'CALM', 'DEVOUT'),
    ('LEADER_JAC_YAMANOUCHI_UESUGI_NORIMASA', 11, 1, 'HEAD', 'LOYAL', 'MUNDANE'),
    ('LEADER_JAC_TAKEDA_NOBUTORA', 40, 0, 'HEAD', 'BOLD', 'VALIANT'),
    ('LEADER_JAC_NAGAO_TAMEKAGE', NULL, 1, 'HEAD', 'BOLD', 'VALIANT'),
    ('LEADER_JAC_IMAGAWA_UJITERU', 21, 1, 'HEAD', 'CAREFUL', 'DEVOUT'),
    ('LEADER_JAC_ODA_NOBUHIDE', 23, 0, 'HEAD', 'BOLD', 'ASTUTE'),
    ('LEADER_JAC_MATSUDAIRA_KIYOYASU', 23, 0, 'HEAD', 'BRAVE', 'VALIANT'),
    ('LEADER_JAC_ASAKURA_TAKAKAGE', 41, 1, 'HEAD', 'CALM', 'DEVOUT'),
    ('LEADER_JAC_ASHIKAGA_YOSHIHARU', 23, 2, 'SHOGUN', 'LOYAL', 'DEVOUT'),
    ('LEADER_JAC_MIYOSHI_NAGAYOSHI', 12, 0, 'HEAD', 'CAREFUL', 'ASTUTE'),
    ('LEADER_JAC_ROKKAKU_SADAYORI', 39, 1, 'HEAD', 'CALM', 'DEVOUT'),
    ('LEADER_JAC_HONGANJI_SHONYO', 18, 2, 'RELIGIOUS_HEAD', 'GENTLE', 'ASCETIC'),
    ('LEADER_JAC_OUCHI_YOSHITAKA', 27, 1, 'HEAD', 'JOLLY', 'DEVOUT'),
    ('LEADER_JAC_AMAGO_TSUNEHISA', 76, 1, 'HEAD', 'CALM', 'ASTUTE'),
    ('LEADER_JAC_MORI_MOTONARI', 37, 0, 'HEAD', 'CAREFUL', 'ASTUTE'),
    ('LEADER_JAC_CHOSOKABE_KUNICHIKA', 30, 0, 'HEAD', 'BRAVE', 'MUNDANE'),
    ('LEADER_JAC_OTOMO_YOSHIAKI', 32, 1, 'HEAD', 'CALM', 'DEVOUT'),
    ('LEADER_JAC_SHIMAZU_TAKAHISA', 20, 0, 'HEAD', 'BRAVE', 'VALIANT'),
    ('LEADER_JAC_RYUKYU_SHO_SEI', 37, 2, 'KING', 'GENTLE', 'DEVOUT'),
    ('LEADER_JAC_AINU_TARIKONA', NULL, 0, 'REPRESENTATIVE', 'CAREFUL', 'MUNDANE');

INSERT INTO JAC_LeaderPersonas
SELECT l.LeaderType, c.CivilizationType,
       'GREAT_PERSON_INDIVIDUAL_JAC_PERSONA_' || substr(l.LeaderType, 12),
       'GOVERNOR_JAC_PERSONA_' || substr(l.LeaderType, 12), l.Name, 'ICON_' || l.LeaderType,
       'GOVERNOR_PROMOTION_JAC_PERSONA_' || substr(l.LeaderType, 12) || '_BASE',
       d.Personality, d.Trait, d.Age1534, d.FameTier, d.Role1534, 1
FROM Leaders l JOIN CivilizationLeaders c ON c.LeaderType=l.LeaderType
JOIN JAC_PersonaDesigns d ON d.LeaderType=l.LeaderType;

UPDATE JAC_LeaderPersonas SET EvolutionCost = MIN(
    (SELECT Value FROM JAC_PersonaCostParameters WHERE Name='MAX_TITLES'), MAX(
    (SELECT Value FROM JAC_PersonaCostParameters WHERE Name='MIN_TITLES'),
    (SELECT Value FROM JAC_PersonaCostParameters WHERE Name='BASE_TITLES') +
    FameTier * (SELECT Value FROM JAC_PersonaCostParameters WHERE Name='FAME_TITLE_WEIGHT') +
    CASE WHEN Age1534 < (SELECT Value FROM JAC_PersonaCostParameters WHERE Name='YOUTH_AGE_LIMIT')
         THEN (SELECT Value FROM JAC_PersonaCostParameters WHERE Name='YOUTH_TITLE_SURCHARGE') ELSE 0 END +
    CASE WHEN Role1534='REPRESENTATIVE'
         THEN (SELECT Value FROM JAC_PersonaCostParameters WHERE Name='REPRESENTATIVE_TITLE_SURCHARGE') ELSE 0 END));

INSERT INTO Types (Type, Kind) VALUES ('GREAT_PERSON_CLASS_JAC_PERSONA', 'KIND_GREAT_PERSON_CLASS');
INSERT INTO Types (Type, Kind) SELECT PersonType, 'KIND_GREAT_PERSON_INDIVIDUAL' FROM JAC_LeaderPersonas;
INSERT INTO Types (Type, Kind) SELECT GovernorType, 'KIND_GOVERNOR' FROM JAC_LeaderPersonas;
-- Native Great People resolve their class from the unit type. Keep a dedicated
-- type, as all nine official classes do (GreatPeople.xml:20-28).
INSERT INTO Types (Type, Kind) VALUES ('UNIT_JAC_PERSONA', 'KIND_UNIT');
INSERT INTO Units (UnitType, Name, Description, Cost, BaseMoves, BaseSightRange,
                   ZoneOfControl, Domain, FormationClass, CanCapture,
                   CanRetreatWhenCaptured, CanTrain)
SELECT 'UNIT_JAC_PERSONA', 'LOC_JAC_PERSONA_CLASS', Description, Cost, BaseMoves,
       BaseSightRange, ZoneOfControl, Domain, FormationClass, CanCapture,
       CanRetreatWhenCaptured, CanTrain FROM Units WHERE UnitType='UNIT_GREAT_GENERAL';
INSERT INTO TypeTags (Type, Tag) VALUES ('UNIT_JAC_PERSONA', 'CLASS_LANDCIVILIAN');
-- Match the official hidden Comandante class defaults (GranColombia_Maya_GreatPeople.xml:24).
-- Joining still enforces world uniqueness; this class never appears in recruitment.
INSERT INTO GreatPersonClasses
    (GreatPersonClassType, Name, UnitType, DistrictType, IconString, ActionIcon, AvailableInTimeline,
     GenerateDuplicateIndividuals, PseudoYieldType)
VALUES ('GREAT_PERSON_CLASS_JAC_PERSONA', 'LOC_JAC_PERSONA_CLASS', 'UNIT_JAC_PERSONA',
        'DISTRICT_CITY_CENTER', '[ICON_GreatGeneral]', 'ICON_UNITOPERATION_GENERAL_ACTION', 0,
        1, 'PSEUDOYIELD_GPP_GENERAL');
INSERT INTO GreatPersonIndividuals
    (GreatPersonIndividualType, Name, GreatPersonClassType, EraType, ActionCharges, Gender)
SELECT PersonType, Name, 'GREAT_PERSON_CLASS_JAC_PERSONA', 'ERA_ANCIENT', 0, 'M' FROM JAC_LeaderPersonas;

-- Native governors own assignment, establishment and the base ability. Custom promotions use merit.
-- Expansion1_Governors.xml:69-75: TransitionStrength 100; IdentityPressure 8.
INSERT INTO Governors
    (GovernorType, Name, Description, Title, ShortTitle, IdentityPressure, TransitionStrength,
     AssignCityState, Image, PortraitImage, PortraitImageSelected)
SELECT GovernorType, Name, 'LOC_JAC_PERSONA_' || substr(LeaderType,12) || '_DESCRIPTION',
       'LOC_JAC_PERSONA_GOVERNOR_TITLE', 'LOC_JAC_PERSONA_GOVERNOR_TITLE',
       8, 100, 0, 'GOVERNOR_EDUCATOR', 'GovernorNormal_Educator', 'GovernorSelected_Educator'
FROM JAC_LeaderPersonas;

-- The native sidecar may open a persona only after synchronized admission.
-- A valid but unattached trait keeps all 22 closed if the sidecar is absent.
-- It is never assigned through LeaderTraits or CivilizationTraits.
INSERT INTO Types (Type, Kind)
VALUES ('TRAIT_JAC_PERSONA_ADMISSION_CLOSED', 'KIND_TRAIT');
INSERT INTO Traits (TraitType, Name, Description)
VALUES ('TRAIT_JAC_PERSONA_ADMISSION_CLOSED', 'LOC_JAC_PERSONA_CLASS', 'LOC_JAC_PERSONA_CLASS');
UPDATE Governors SET TraitType = 'TRAIT_JAC_PERSONA_ADMISSION_CLOSED'
WHERE GovernorType IN (SELECT GovernorType FROM JAC_LeaderPersonas);

CREATE TABLE JAC_PersonaSkills (
    LeaderType TEXT NOT NULL REFERENCES JAC_LeaderPersonas(LeaderType),
    Slot INTEGER NOT NULL CHECK (Slot = 0),
    GovernorPromotionType TEXT NOT NULL UNIQUE,
    ModifierId TEXT NOT NULL UNIQUE, ModifierType TEXT NOT NULL REFERENCES DynamicModifiers(ModifierType),
    YieldType TEXT REFERENCES Yields(YieldType), Amount INTEGER NOT NULL,
    PRIMARY KEY (LeaderType, Slot)
);
-- One native base per person. Shared routes and signatures are in JAC_LeaderPersona_Trees.sql.
INSERT INTO JAC_PersonaSkills VALUES
    ('LEADER_JAC_DATE_TANEMUNE', 0, 'GOVERNOR_PROMOTION_JAC_PERSONA_DATE_TANEMUNE_BASE', 'JAC_PERSONA_DATE_TANEMUNE_BASE', 'MODIFIER_SINGLE_CITY_ADJUST_CITY_YIELD_MODIFIER', 'YIELD_CULTURE', 10),
    ('LEADER_JAC_ASHINA_MORIKIYO', 0, 'GOVERNOR_PROMOTION_JAC_PERSONA_ASHINA_MORIKIYO_BASE', 'JAC_PERSONA_ASHINA_MORIKIYO_BASE', 'MODIFIER_SINGLE_CITY_ADJUST_CITY_YIELD_MODIFIER', 'YIELD_FOOD', 10),
    ('LEADER_JAC_HOJO_UJITSUNA', 0, 'GOVERNOR_PROMOTION_JAC_PERSONA_HOJO_UJITSUNA_BASE', 'JAC_PERSONA_HOJO_UJITSUNA_BASE', 'MODIFIER_SINGLE_CITY_ADJUST_CITY_YIELD_MODIFIER', 'YIELD_GOLD', 10),
    ('LEADER_JAC_YAMANOUCHI_UESUGI_NORIMASA', 0, 'GOVERNOR_PROMOTION_JAC_PERSONA_YAMANOUCHI_UESUGI_NORIMASA_BASE', 'JAC_PERSONA_YAMANOUCHI_UESUGI_NORIMASA_BASE', 'MODIFIER_SINGLE_CITY_ADJUST_CITY_YIELD_MODIFIER', 'YIELD_CULTURE', 10),
    ('LEADER_JAC_TAKEDA_NOBUTORA', 0, 'GOVERNOR_PROMOTION_JAC_PERSONA_TAKEDA_NOBUTORA_BASE', 'JAC_PERSONA_TAKEDA_NOBUTORA_BASE', 'MODIFIER_GOVERNOR_ADJUST_CITY_MILITARY_UNIT_PRODUCTION', NULL, 20),
    ('LEADER_JAC_NAGAO_TAMEKAGE', 0, 'GOVERNOR_PROMOTION_JAC_PERSONA_NAGAO_TAMEKAGE_BASE', 'JAC_PERSONA_NAGAO_TAMEKAGE_BASE', 'MODIFIER_GOVERNOR_ADJUST_CITY_MILITARY_UNIT_PRODUCTION', NULL, 20),
    ('LEADER_JAC_IMAGAWA_UJITERU', 0, 'GOVERNOR_PROMOTION_JAC_PERSONA_IMAGAWA_UJITERU_BASE', 'JAC_PERSONA_IMAGAWA_UJITERU_BASE', 'MODIFIER_SINGLE_CITY_ADJUST_CITY_YIELD_MODIFIER', 'YIELD_CULTURE', 10),
    ('LEADER_JAC_ODA_NOBUHIDE', 0, 'GOVERNOR_PROMOTION_JAC_PERSONA_ODA_NOBUHIDE_BASE', 'JAC_PERSONA_ODA_NOBUHIDE_BASE', 'MODIFIER_SINGLE_CITY_ADJUST_CITY_YIELD_MODIFIER', 'YIELD_GOLD', 10),
    ('LEADER_JAC_MATSUDAIRA_KIYOYASU', 0, 'GOVERNOR_PROMOTION_JAC_PERSONA_MATSUDAIRA_KIYOYASU_BASE', 'JAC_PERSONA_MATSUDAIRA_KIYOYASU_BASE', 'MODIFIER_GOVERNOR_ADJUST_CITY_MILITARY_UNIT_PRODUCTION', NULL, 20),
    ('LEADER_JAC_ASAKURA_TAKAKAGE', 0, 'GOVERNOR_PROMOTION_JAC_PERSONA_ASAKURA_TAKAKAGE_BASE', 'JAC_PERSONA_ASAKURA_TAKAKAGE_BASE', 'MODIFIER_SINGLE_CITY_ADJUST_CITY_YIELD_MODIFIER', 'YIELD_CULTURE', 10),
    ('LEADER_JAC_ASHIKAGA_YOSHIHARU', 0, 'GOVERNOR_PROMOTION_JAC_PERSONA_ASHIKAGA_YOSHIHARU_BASE', 'JAC_PERSONA_ASHIKAGA_YOSHIHARU_BASE', 'MODIFIER_SINGLE_CITY_ADJUST_CITY_YIELD_MODIFIER', 'YIELD_CULTURE', 10),
    ('LEADER_JAC_MIYOSHI_NAGAYOSHI', 0, 'GOVERNOR_PROMOTION_JAC_PERSONA_MIYOSHI_NAGAYOSHI_BASE', 'JAC_PERSONA_MIYOSHI_NAGAYOSHI_BASE', 'MODIFIER_SINGLE_CITY_ADJUST_CITY_YIELD_MODIFIER', 'YIELD_PRODUCTION', 10),
    ('LEADER_JAC_ROKKAKU_SADAYORI', 0, 'GOVERNOR_PROMOTION_JAC_PERSONA_ROKKAKU_SADAYORI_BASE', 'JAC_PERSONA_ROKKAKU_SADAYORI_BASE', 'MODIFIER_SINGLE_CITY_ADJUST_CITY_YIELD_MODIFIER', 'YIELD_GOLD', 10),
    ('LEADER_JAC_HONGANJI_SHONYO', 0, 'GOVERNOR_PROMOTION_JAC_PERSONA_HONGANJI_SHONYO_BASE', 'JAC_PERSONA_HONGANJI_SHONYO_BASE', 'MODIFIER_SINGLE_CITY_ADJUST_CITY_YIELD_MODIFIER', 'YIELD_FAITH', 10),
    ('LEADER_JAC_OUCHI_YOSHITAKA', 0, 'GOVERNOR_PROMOTION_JAC_PERSONA_OUCHI_YOSHITAKA_BASE', 'JAC_PERSONA_OUCHI_YOSHITAKA_BASE', 'MODIFIER_SINGLE_CITY_ADJUST_CITY_YIELD_MODIFIER', 'YIELD_CULTURE', 10),
    ('LEADER_JAC_AMAGO_TSUNEHISA', 0, 'GOVERNOR_PROMOTION_JAC_PERSONA_AMAGO_TSUNEHISA_BASE', 'JAC_PERSONA_AMAGO_TSUNEHISA_BASE', 'MODIFIER_SINGLE_CITY_ADJUST_CITY_YIELD_MODIFIER', 'YIELD_PRODUCTION', 10),
    ('LEADER_JAC_MORI_MOTONARI', 0, 'GOVERNOR_PROMOTION_JAC_PERSONA_MORI_MOTONARI_BASE', 'JAC_PERSONA_MORI_MOTONARI_BASE', 'MODIFIER_SINGLE_CITY_ADJUST_CITY_YIELD_MODIFIER', 'YIELD_FOOD', 10),
    ('LEADER_JAC_CHOSOKABE_KUNICHIKA', 0, 'GOVERNOR_PROMOTION_JAC_PERSONA_CHOSOKABE_KUNICHIKA_BASE', 'JAC_PERSONA_CHOSOKABE_KUNICHIKA_BASE', 'MODIFIER_SINGLE_CITY_ADJUST_CITY_YIELD_MODIFIER', 'YIELD_FOOD', 10),
    ('LEADER_JAC_OTOMO_YOSHIAKI', 0, 'GOVERNOR_PROMOTION_JAC_PERSONA_OTOMO_YOSHIAKI_BASE', 'JAC_PERSONA_OTOMO_YOSHIAKI_BASE', 'MODIFIER_SINGLE_CITY_ADJUST_CITY_YIELD_MODIFIER', 'YIELD_GOLD', 10),
    ('LEADER_JAC_SHIMAZU_TAKAHISA', 0, 'GOVERNOR_PROMOTION_JAC_PERSONA_SHIMAZU_TAKAHISA_BASE', 'JAC_PERSONA_SHIMAZU_TAKAHISA_BASE', 'MODIFIER_GOVERNOR_ADJUST_CITY_MILITARY_UNIT_PRODUCTION', NULL, 20),
    ('LEADER_JAC_RYUKYU_SHO_SEI', 0, 'GOVERNOR_PROMOTION_JAC_PERSONA_RYUKYU_SHO_SEI_BASE', 'JAC_PERSONA_RYUKYU_SHO_SEI_BASE', 'MODIFIER_SINGLE_CITY_ADJUST_CITY_YIELD_MODIFIER', 'YIELD_GOLD', 10),
    ('LEADER_JAC_AINU_TARIKONA', 0, 'GOVERNOR_PROMOTION_JAC_PERSONA_AINU_TARIKONA_BASE', 'JAC_PERSONA_AINU_TARIKONA_BASE', 'MODIFIER_SINGLE_CITY_ADJUST_CITY_YIELD_MODIFIER', 'YIELD_FOOD', 10);

INSERT INTO Types (Type, Kind) SELECT GovernorPromotionType, 'KIND_GOVERNOR_PROMOTION' FROM JAC_PersonaSkills;
INSERT INTO GovernorPromotions (GovernorPromotionType, Name, Description, Level, "Column", BaseAbility)
SELECT GovernorPromotionType, 'LOC_' || GovernorPromotionType || '_NAME',
       'LOC_' || GovernorPromotionType || '_DESCRIPTION',
       0, 1, 1 FROM JAC_PersonaSkills;
INSERT INTO GovernorPromotionSets (GovernorType, GovernorPromotion)
SELECT p.GovernorType, s.GovernorPromotionType FROM JAC_PersonaSkills s JOIN JAC_LeaderPersonas p USING (LeaderType);
INSERT INTO Modifiers (ModifierId, ModifierType) SELECT ModifierId, ModifierType FROM JAC_PersonaSkills;
INSERT INTO ModifierArguments (ModifierId, Name, Value) SELECT ModifierId, 'Amount', Amount FROM JAC_PersonaSkills;
INSERT INTO ModifierArguments (ModifierId, Name, Value) SELECT ModifierId, 'YieldType', YieldType FROM JAC_PersonaSkills WHERE YieldType IS NOT NULL;
INSERT INTO GovernorPromotionModifiers (GovernorPromotionType, ModifierId) SELECT GovernorPromotionType, ModifierId FROM JAC_PersonaSkills;
-- The native base promotion marks its active city for GameCore merit checks.
-- Establishment, neutralization and reassignment behavior need a live probe.
INSERT INTO Modifiers (ModifierId, ModifierType)
SELECT 'JAC_PERSONA_SETTLED_' || LeaderType, 'MODIFIER_SINGLE_CITY_ADJUST_PROPERTY'
FROM JAC_LeaderPersonas;
INSERT INTO ModifierArguments (ModifierId, Name, Value)
SELECT 'JAC_PERSONA_SETTLED_' || LeaderType, 'Key', 'JAC_PERSONA_SETTLED_' || LeaderType
FROM JAC_LeaderPersonas;
INSERT INTO ModifierArguments (ModifierId, Name, Value)
SELECT 'JAC_PERSONA_SETTLED_' || LeaderType, 'Amount', 1
FROM JAC_LeaderPersonas;
INSERT INTO GovernorPromotionModifiers (GovernorPromotionType, ModifierId)
SELECT BasePromotionType, 'JAC_PERSONA_SETTLED_' || LeaderType
FROM JAC_LeaderPersonas;
-- APPOINT_GOVERNOR spends the first title.
-- BEGIN RUST PERSONA PAYMENT DEFINITIONS
-- Generated by civ6-data-rs::persona_payments. Prices never remove these rows.
-- The frozen d21 prefix preserves existing Index allocation in rebuilt DBs.
CREATE TABLE JAC_PersonaLegacyPaymentCosts (
    LeaderType TEXT PRIMARY KEY REFERENCES JAC_LeaderPersonas(LeaderType),
    Cost INTEGER NOT NULL CHECK (Cost BETWEEN 1 AND 3)
);
INSERT INTO JAC_PersonaLegacyPaymentCosts VALUES
    ('LEADER_JAC_DATE_TANEMUNE', 2),
    ('LEADER_JAC_ASHINA_MORIKIYO', 1),
    ('LEADER_JAC_HOJO_UJITSUNA', 2),
    ('LEADER_JAC_YAMANOUCHI_UESUGI_NORIMASA', 3),
    ('LEADER_JAC_TAKEDA_NOBUTORA', 1),
    ('LEADER_JAC_NAGAO_TAMEKAGE', 2),
    ('LEADER_JAC_IMAGAWA_UJITERU', 2),
    ('LEADER_JAC_ODA_NOBUHIDE', 1),
    ('LEADER_JAC_MATSUDAIRA_KIYOYASU', 1),
    ('LEADER_JAC_ASAKURA_TAKAKAGE', 2),
    ('LEADER_JAC_ASHIKAGA_YOSHIHARU', 3),
    ('LEADER_JAC_MIYOSHI_NAGAYOSHI', 2),
    ('LEADER_JAC_ROKKAKU_SADAYORI', 2),
    ('LEADER_JAC_HONGANJI_SHONYO', 3),
    ('LEADER_JAC_OUCHI_YOSHITAKA', 2),
    ('LEADER_JAC_AMAGO_TSUNEHISA', 2),
    ('LEADER_JAC_MORI_MOTONARI', 1),
    ('LEADER_JAC_CHOSOKABE_KUNICHIKA', 1),
    ('LEADER_JAC_OTOMO_YOSHIAKI', 2),
    ('LEADER_JAC_SHIMAZU_TAKAHISA', 1),
    ('LEADER_JAC_RYUKYU_SHO_SEI', 3),
    ('LEADER_JAC_AINU_TARIKONA', 2);
CREATE TABLE JAC_PersonaTitlePayments (
    LeaderType TEXT NOT NULL REFERENCES JAC_LeaderPersonas(LeaderType),
    Step INTEGER NOT NULL CHECK (Step IN (1, 2)),
    GovernorPromotionType TEXT NOT NULL UNIQUE,
    PRIMARY KEY (LeaderType, Step)
);
INSERT INTO JAC_PersonaTitlePayments
SELECT p.LeaderType, 1, p.BasePromotionType || '_TITLE_1'
FROM JAC_LeaderPersonas p JOIN JAC_PersonaLegacyPaymentCosts l USING (LeaderType)
WHERE l.Cost >= 2 ORDER BY p.rowid;
INSERT INTO JAC_PersonaTitlePayments
SELECT p.LeaderType, 2, p.BasePromotionType || '_TITLE_2'
FROM JAC_LeaderPersonas p JOIN JAC_PersonaLegacyPaymentCosts l USING (LeaderType)
WHERE l.Cost >= 3 ORDER BY p.rowid;
-- Keep the original Types allocation before any new payment type is appended.
INSERT INTO Types (Type, Kind)
SELECT GovernorPromotionType, 'KIND_GOVERNOR_PROMOTION' FROM JAC_PersonaTitlePayments;
INSERT INTO GovernorPromotions (GovernorPromotionType, Name, Description, Level, "Column", BaseAbility)
SELECT GovernorPromotionType, 'LOC_JAC_PERSONA_TITLE_PAYMENT_NAME',
       'LOC_JAC_PERSONA_TITLE_PAYMENT_DESCRIPTION', Step, 1, 0
FROM JAC_PersonaTitlePayments ORDER BY rowid;
INSERT INTO GovernorPromotionSets (GovernorType, GovernorPromotion)
SELECT p.GovernorType, t.GovernorPromotionType
FROM JAC_PersonaTitlePayments t JOIN JAC_LeaderPersonas p USING (LeaderType);
INSERT INTO GovernorPromotionPrereqs (GovernorPromotionType, PrereqGovernorPromotion)
SELECT t.GovernorPromotionType,
       CASE WHEN t.Step = 1 THEN p.BasePromotionType ELSE p.BasePromotionType || '_TITLE_1' END
FROM JAC_PersonaTitlePayments t JOIN JAC_LeaderPersonas p USING (LeaderType);
-- Append the rest of the fixed 1..3 payment capacity after the legacy prefix.
-- New joins use the current formula; owned people use only their saved prefix.
INSERT INTO JAC_PersonaTitlePayments
SELECT p.LeaderType, 1, p.BasePromotionType || '_TITLE_1'
FROM JAC_LeaderPersonas p JOIN JAC_PersonaLegacyPaymentCosts l USING (LeaderType)
WHERE l.Cost < 2 ORDER BY p.rowid;
INSERT INTO JAC_PersonaTitlePayments
SELECT p.LeaderType, 2, p.BasePromotionType || '_TITLE_2'
FROM JAC_LeaderPersonas p JOIN JAC_PersonaLegacyPaymentCosts l USING (LeaderType)
WHERE l.Cost < 3 ORDER BY p.rowid;
INSERT INTO Types (Type, Kind)
SELECT t.GovernorPromotionType, 'KIND_GOVERNOR_PROMOTION' FROM JAC_PersonaTitlePayments t
WHERE NOT EXISTS (SELECT 1 FROM Types x WHERE x.Type=t.GovernorPromotionType);
INSERT INTO GovernorPromotions (GovernorPromotionType, Name, Description, Level, "Column", BaseAbility)
SELECT t.GovernorPromotionType, 'LOC_JAC_PERSONA_TITLE_PAYMENT_NAME',
       'LOC_JAC_PERSONA_TITLE_PAYMENT_DESCRIPTION', t.Step, 1, 0
FROM JAC_PersonaTitlePayments t
WHERE NOT EXISTS (SELECT 1 FROM GovernorPromotions x WHERE x.GovernorPromotionType=t.GovernorPromotionType)
ORDER BY t.rowid;
INSERT INTO GovernorPromotionSets (GovernorType, GovernorPromotion)
SELECT p.GovernorType, t.GovernorPromotionType
FROM JAC_PersonaTitlePayments t JOIN JAC_LeaderPersonas p USING (LeaderType)
WHERE NOT EXISTS (SELECT 1 FROM GovernorPromotionSets x WHERE x.GovernorType=p.GovernorType AND x.GovernorPromotion=t.GovernorPromotionType);
INSERT INTO GovernorPromotionPrereqs (GovernorPromotionType, PrereqGovernorPromotion)
SELECT t.GovernorPromotionType,
       CASE WHEN t.Step = 1 THEN p.BasePromotionType ELSE p.BasePromotionType || '_TITLE_1' END
FROM JAC_PersonaTitlePayments t JOIN JAC_LeaderPersonas p USING (LeaderType)
WHERE NOT EXISTS (SELECT 1 FROM GovernorPromotionPrereqs x WHERE x.GovernorPromotionType=t.GovernorPromotionType);
-- END RUST PERSONA PAYMENT DEFINITIONS
UPDATE GlobalParameters SET Value=MAX(CAST(Value AS INTEGER), 30) WHERE Name='MAX_GOVERNOR_APPOINTMENTS';

-- This map-scoped component disables only the eight standard official governors.
-- An unattached synthetic TraitType crashed CanEverAppointGovernor in live runs
-- leaderperson-01/02. Remove official candidates rather than make native trait
-- checks dereference it. Native governor child tables cascade on deletion
-- (Expansion2_Schema.sql:337-370, GovernorPromotionSets).
CREATE TABLE JAC_PersonaDisabledGovernors (GovernorType TEXT PRIMARY KEY);
INSERT INTO JAC_PersonaDisabledGovernors
SELECT GovernorType FROM Governors WHERE GovernorType IN (
 'GOVERNOR_THE_DEFENDER', 'GOVERNOR_THE_CARDINAL', 'GOVERNOR_THE_BUILDER',
 'GOVERNOR_THE_AMBASSADOR', 'GOVERNOR_THE_RESOURCE_MANAGER', 'GOVERNOR_THE_MERCHANT',
 'GOVERNOR_THE_EDUCATOR', 'GOVERNOR_IBRAHIM');
DELETE FROM Governors WHERE GovernorType IN (SELECT GovernorType FROM JAC_PersonaDisabledGovernors);

-- Official shape: DLC/Expansion2/Data/Expansion1_Civics.xml:64-66,77-80,90-140.
-- That installed GS file grants 13 civics; this provisional Sengoku set has 15.
CREATE TABLE JAC_PersonaTitleCivics (CivicType TEXT PRIMARY KEY REFERENCES Civics(CivicType));
INSERT INTO JAC_PersonaTitleCivics VALUES
 ('CIVIC_JAC_SENGOKU_DOMAIN_LAW'),
 ('CIVIC_JAC_SENGOKU_VILLAGE_ASSEMBLIES'),
 ('CIVIC_JAC_SENGOKU_LEARNING_AND_RECORDS'),
 ('CIVIC_JAC_SENGOKU_MILITARY_SERVICE_LEDGER'),
 ('CIVIC_JAC_SENGOKU_OATHS_AND_ALLIANCES'),
 ('CIVIC_JAC_SENGOKU_PORT_MERCHANT_COUNCILS'),
 ('CIVIC_JAC_SENGOKU_CASTLE_TOWN_ORDINANCES'),
 ('CIVIC_JAC_SENGOKU_COURT_AND_SHOGUNATE_ENVOYS'),
 ('CIVIC_JAC_SENGOKU_SHRINE_LAND_GUARANTEES'),
 ('CIVIC_JAC_SENGOKU_DOMAIN_LAND_SURVEYS'),
 ('CIVIC_JAC_SENGOKU_TRUCE_ARBITRATION'),
 ('CIVIC_JAC_SENGOKU_TAIKO_LAND_SURVEY'),
 ('CIVIC_JAC_SENGOKU_RED_SEAL_WRITS'),
 ('CIVIC_JAC_SENGOKU_PORT_ADMINISTRATION'),
 ('CIVIC_JAC_SENGOKU_LAND_TENURE_CONFIRMATION');
INSERT INTO Modifiers (ModifierId, ModifierType)
VALUES ('JAC_PERSONA_CIVIC_EVOLUTION_TITLE', 'MODIFIER_PLAYER_ADJUST_GOVERNOR_POINTS');
INSERT INTO ModifierArguments (ModifierId, Name, Value)
VALUES ('JAC_PERSONA_CIVIC_EVOLUTION_TITLE', 'Delta', 1);
INSERT INTO CivicModifiers (CivicType, ModifierId)
SELECT CivicType, 'JAC_PERSONA_CIVIC_EVOLUTION_TITLE' FROM JAC_PersonaTitleCivics;
