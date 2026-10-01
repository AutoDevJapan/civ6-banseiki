-- Diagnostic only. This candidate is not the completed persona implementation.
-- GovernorsCannotAssign: Expansion2_Schema.sql:362; official UI calls these
-- "secret governors". AI abstention is unproven and MUST be measured in game.
DELETE FROM Governors WHERE GovernorType IN (SELECT GovernorType FROM JAC_LeaderPersonas);
DELETE FROM Types WHERE Type IN (SELECT GovernorType FROM JAC_LeaderPersonas);
INSERT INTO Types (Type, Kind) VALUES
 ('GOVERNOR_JAC_TITLE_PROXY', 'KIND_GOVERNOR'),
 ('GOVERNOR_PROMOTION_JAC_TITLE_PROXY_BASE', 'KIND_GOVERNOR_PROMOTION');
INSERT INTO Governors (GovernorType, Name, Description, Title, ShortTitle,
 IdentityPressure, TransitionStrength, AssignCityState, Image, PortraitImage, PortraitImageSelected)
VALUES ('GOVERNOR_JAC_TITLE_PROXY', 'LOC_JAC_PERSONA_TITLE_PAYMENT_NAME',
 'LOC_JAC_PERSONA_TITLE_PAYMENT_DESCRIPTION', 'LOC_JAC_PERSONA_TITLE_PAYMENT_NAME',
 'LOC_JAC_PERSONA_TITLE_PAYMENT_NAME', 0, 100, 0,
 'GOVERNOR_EDUCATOR', 'GovernorNormal_Educator', 'GovernorSelected_Educator');
INSERT INTO GovernorsCannotAssign (GovernorType, CannotAssign) VALUES ('GOVERNOR_JAC_TITLE_PROXY', 1);
INSERT INTO GovernorPromotions (GovernorPromotionType, Name, Description, Level, "Column", BaseAbility)
VALUES ('GOVERNOR_PROMOTION_JAC_TITLE_PROXY_BASE', 'LOC_JAC_PERSONA_TITLE_PAYMENT_NAME',
 'LOC_JAC_PERSONA_TITLE_PAYMENT_DESCRIPTION', 0, 0, 1);
INSERT INTO GovernorPromotionSets (GovernorType, GovernorPromotion) VALUES ('GOVERNOR_JAC_TITLE_PROXY', 'GOVERNOR_PROMOTION_JAC_TITLE_PROXY_BASE');
CREATE TABLE JAC_TitleProxyPayments (Step INTEGER PRIMARY KEY, PromotionType TEXT NOT NULL UNIQUE);
WITH RECURSIVE steps(n) AS (SELECT 1 UNION ALL SELECT n+1 FROM steps WHERE n<65)
INSERT INTO JAC_TitleProxyPayments SELECT n, 'GOVERNOR_PROMOTION_JAC_TITLE_PROXY_' || n FROM steps;
INSERT INTO Types (Type, Kind) SELECT PromotionType, 'KIND_GOVERNOR_PROMOTION' FROM JAC_TitleProxyPayments;
INSERT INTO GovernorPromotions (GovernorPromotionType, Name, Description, Level, "Column", BaseAbility)
SELECT PromotionType, 'LOC_JAC_PERSONA_TITLE_PAYMENT_NAME', 'LOC_JAC_PERSONA_TITLE_PAYMENT_DESCRIPTION', Step, 0, 0
FROM JAC_TitleProxyPayments;
INSERT INTO GovernorPromotionSets (GovernorType, GovernorPromotion)
SELECT 'GOVERNOR_JAC_TITLE_PROXY', PromotionType FROM JAC_TitleProxyPayments;
INSERT INTO GovernorPromotionPrereqs (GovernorPromotionType, PrereqGovernorPromotion)
SELECT PromotionType, CASE WHEN Step=1 THEN 'GOVERNOR_PROMOTION_JAC_TITLE_PROXY_BASE'
 ELSE 'GOVERNOR_PROMOTION_JAC_TITLE_PROXY_' || (Step-1) END FROM JAC_TitleProxyPayments;

-- Candidate native receipt. MODIFIER_PLAYER_ADJUST_PROPERTY is registered in
-- the official GS schema (COLLECTION_OWNER / EFFECT_ADJUST_PLAYER_PROPERTY).
-- Ethiopia_SecretSocieties_MODE.xml:198,714 applies player effects from a
-- cannot-assign Governor promotion. Receipt activation must be observed live.
CREATE TABLE JAC_TitleProxyReceipts (Step INTEGER PRIMARY KEY, PromotionType TEXT NOT NULL UNIQUE,
 ModifierId TEXT NOT NULL UNIQUE, PropertyKey TEXT NOT NULL UNIQUE);
INSERT INTO JAC_TitleProxyReceipts VALUES (0, 'GOVERNOR_PROMOTION_JAC_TITLE_PROXY_BASE',
 'JAC_PROXY_RECEIPT_0', 'JAC_PROXY_NATIVE_PAID_0');
INSERT INTO JAC_TitleProxyReceipts
SELECT Step, PromotionType, 'JAC_PROXY_RECEIPT_' || Step, 'JAC_PROXY_NATIVE_PAID_' || Step
FROM JAC_TitleProxyPayments;
INSERT INTO Modifiers (ModifierId, ModifierType)
SELECT ModifierId, 'MODIFIER_PLAYER_ADJUST_PROPERTY' FROM JAC_TitleProxyReceipts;
INSERT INTO ModifierArguments (ModifierId, Name, Value)
SELECT ModifierId, 'Key', PropertyKey FROM JAC_TitleProxyReceipts;
INSERT INTO ModifierArguments (ModifierId, Name, Value)
SELECT ModifierId, 'Amount', 1 FROM JAC_TitleProxyReceipts;
INSERT INTO GovernorPromotionModifiers (GovernorPromotionType, ModifierId)
SELECT PromotionType, ModifierId FROM JAC_TitleProxyReceipts;
