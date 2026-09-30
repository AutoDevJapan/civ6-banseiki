-- Reuse generic infantry/cavalry art during the lifecycle PoC, never vanilla Hero portraits.
INSERT INTO IconDefinitions (Name,"Index",Atlas)
SELECT 'ICON_UNIT_HERO_JAC_WARLORD_A',"Index",Atlas FROM IconDefinitions WHERE Name = 'ICON_UNIT_WARRIOR';
INSERT INTO IconDefinitions (Name,"Index",Atlas)
SELECT 'ICON_UNIT_HERO_JAC_WARLORD_B',"Index",Atlas FROM IconDefinitions WHERE Name = 'ICON_UNIT_HORSEMAN';
INSERT INTO IconDefinitions (Name,"Index",Atlas)
SELECT 'ICON_HEROCLASS_JAC_WARLORD_A',"Index",Atlas FROM IconDefinitions WHERE Name = 'ICON_UNIT_WARRIOR';
INSERT INTO IconDefinitions (Name,"Index",Atlas)
SELECT 'ICON_HEROCLASS_JAC_WARLORD_B',"Index",Atlas FROM IconDefinitions WHERE Name = 'ICON_UNIT_HORSEMAN';
