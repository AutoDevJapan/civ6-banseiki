-- Diagnostic only. This candidate is not the completed persona implementation.
-- GovernorsCannotAssign: Expansion2_Schema.sql:362; official UI calls these
-- "secret governors". AI abstention is unproven and MUST be measured in game.
DELETE FROM Governors WHERE GovernorType IN (SELECT GovernorType FROM JAC_LeaderPersonas);
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
