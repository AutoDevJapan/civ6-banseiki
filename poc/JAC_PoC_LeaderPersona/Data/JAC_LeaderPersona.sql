-- PoC only: SSOT 9.4 (joining -> Great Person -> same-name Governor), 16.1 bundle 2.
-- Loaded at 400, AFTER SengokuTrees removes the official sources of governor titles.
-- Official schema: Base/Assets/Gameplay/Data/Schema/01_GameplaySchema.sql:1502,1519;
-- DLC/Expansion1/Data/Expansion1_Schema.sql:131,160,190.
CREATE TABLE JAC_LeaderPersonas (
    LeaderType TEXT PRIMARY KEY REFERENCES Leaders(LeaderType),
    CivilizationType TEXT NOT NULL UNIQUE REFERENCES Civilizations(CivilizationType),
    PersonType TEXT NOT NULL UNIQUE,
    GovernorType TEXT NOT NULL UNIQUE,
    Name TEXT NOT NULL,
    PortraitIcon TEXT NOT NULL,
    BasePromotionType TEXT NOT NULL UNIQUE,
    PromotionType TEXT NOT NULL UNIQUE
);

INSERT INTO JAC_LeaderPersonas
SELECT l.LeaderType, c.CivilizationType,
       'GREAT_PERSON_INDIVIDUAL_JAC_PERSONA_' || substr(l.LeaderType, 12),
       'GOVERNOR_JAC_PERSONA_' || substr(l.LeaderType, 12), l.Name, 'ICON_' || l.LeaderType,
       'GOVERNOR_PROMOTION_JAC_PERSONA_' || substr(l.LeaderType, 12) || '_BASE',
       'GOVERNOR_PROMOTION_JAC_PERSONA_' || substr(l.LeaderType, 12) || '_TEST'
FROM Leaders l JOIN CivilizationLeaders c ON c.LeaderType=l.LeaderType
WHERE l.LeaderType LIKE 'LEADER_JAC_%' AND c.CivilizationType LIKE 'CIVILIZATION_JAC_%';

INSERT INTO Types (Type, Kind) VALUES ('GREAT_PERSON_CLASS_JAC_PERSONA', 'KIND_GREAT_PERSON_CLASS');
INSERT INTO Types (Type, Kind) SELECT PersonType, 'KIND_GREAT_PERSON_INDIVIDUAL' FROM JAC_LeaderPersonas;
INSERT INTO Types (Type, Kind) SELECT GovernorType, 'KIND_GOVERNOR' FROM JAC_LeaderPersonas;
INSERT INTO Types (Type, Kind) SELECT BasePromotionType, 'KIND_GOVERNOR_PROMOTION' FROM JAC_LeaderPersonas;
INSERT INTO Types (Type, Kind) SELECT PromotionType, 'KIND_GOVERNOR_PROMOTION' FROM JAC_LeaderPersonas;

-- No points, recruitment timeline, action charges or retirement modifiers.
-- The native map model is a Great General; the persona UI uses the leader's existing portrait.
INSERT INTO GreatPersonClasses
    (GreatPersonClassType, Name, UnitType, DistrictType, IconString, ActionIcon, AvailableInTimeline)
VALUES ('GREAT_PERSON_CLASS_JAC_PERSONA', 'LOC_JAC_PERSONA_CLASS', 'UNIT_GREAT_GENERAL',
        'DISTRICT_CITY_CENTER', '[ICON_GreatGeneral]', 'ICON_UNITOPERATION_GENERAL_ACTION', 0);
INSERT INTO GreatPersonIndividuals
    (GreatPersonIndividualType, Name, GreatPersonClassType, EraType, ActionCharges, Gender)
SELECT PersonType, Name, 'GREAT_PERSON_CLASS_JAC_PERSONA', 'ERA_ANCIENT', 0, 'M' FROM JAC_LeaderPersonas;

-- Ordinary assignable Governors, with engine-owned establishment/assignment/promotion/save state.
-- 100 is the normal transition strength, not a number of turns (Expansion1_Governors.xml:69-75).
INSERT INTO Governors
    (GovernorType, Name, Description, Title, ShortTitle, IdentityPressure, TransitionStrength,
     AssignCityState, Image, PortraitImage, PortraitImageSelected)
SELECT GovernorType, Name, 'LOC_JAC_PERSONA_GOVERNOR_DESCRIPTION', 'LOC_JAC_PERSONA_GOVERNOR_TITLE',
       'LOC_JAC_PERSONA_GOVERNOR_TITLE', 8, 100, 0, 'GOVERNOR_EDUCATOR',
       'GovernorNormal_Educator', 'GovernorSelected_Educator' FROM JAC_LeaderPersonas;

-- Each person has its own two-node test tree. No invented historical abilities or yields.
INSERT INTO GovernorPromotions (GovernorPromotionType, Name, Description, Level, "Column", BaseAbility)
SELECT BasePromotionType, 'LOC_JAC_PERSONA_BASE', 'LOC_JAC_PERSONA_SKILL_DESCRIPTION', 0, 1, 1
FROM JAC_LeaderPersonas;
INSERT INTO GovernorPromotions (GovernorPromotionType, Name, Description, Level, "Column", BaseAbility)
SELECT PromotionType, 'LOC_JAC_PERSONA_PROMOTION', 'LOC_JAC_PERSONA_SKILL_DESCRIPTION', 1, 1, 0
FROM JAC_LeaderPersonas;
INSERT INTO GovernorPromotionSets SELECT GovernorType, BasePromotionType FROM JAC_LeaderPersonas;
INSERT INTO GovernorPromotionSets SELECT GovernorType, PromotionType FROM JAC_LeaderPersonas;
INSERT INTO GovernorPromotionPrereqs SELECT PromotionType, BasePromotionType FROM JAC_LeaderPersonas;

-- The roster needs up to 22 persona slots, alongside the existing 8 (Expansion2_GlobalParameters.xml:200).
UPDATE GlobalParameters SET Value=MAX(CAST(Value AS INTEGER), 30) WHERE Name='MAX_GOVERNOR_APPOINTMENTS';
