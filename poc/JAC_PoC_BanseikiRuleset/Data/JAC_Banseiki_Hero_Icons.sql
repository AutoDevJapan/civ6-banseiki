-- Reuse generic infantry/cavalry art during the lifecycle PoC, never vanilla Hero portraits.
INSERT INTO IconDefinitions (Name,Size,"Index",Atlas)
SELECT 'ICON_UNIT_HERO_JAC_WARLORD_A',Size,"Index",Atlas FROM IconDefinitions WHERE Name = 'ICON_UNIT_WARRIOR';
INSERT INTO IconDefinitions (Name,Size,"Index",Atlas)
SELECT 'ICON_UNIT_HERO_JAC_WARLORD_B',Size,"Index",Atlas FROM IconDefinitions WHERE Name = 'ICON_UNIT_HORSEMAN';
INSERT INTO IconDefinitions (Name,Size,"Index",Atlas)
SELECT 'ICON_HEROCLASS_JAC_WARLORD_A',Size,"Index",Atlas FROM IconDefinitions WHERE Name = 'ICON_UNIT_WARRIOR';
INSERT INTO IconDefinitions (Name,Size,"Index",Atlas)
SELECT 'ICON_HEROCLASS_JAC_WARLORD_B',Size,"Index",Atlas FROM IconDefinitions WHERE Name = 'ICON_UNIT_HORSEMAN';
