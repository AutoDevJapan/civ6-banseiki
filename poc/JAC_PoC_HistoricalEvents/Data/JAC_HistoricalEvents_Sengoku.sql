-- Sengoku pack sample events. Values are provisional (暫定); see documents/poc/historical-events.md.

-- Year -> turn: Scripts/JAC_HistoricalEvents.lua converts Year with the game speed's GameSpeed_Turns rows
-- (SengokuTrees calendar, 1534 start; standard speed = 3 turns per year).

INSERT INTO JAC_HistoricalEvents (EventType, Pack, Year, PrereqTech, PrereqCivic, Name, Description, NotifyAll) VALUES
	('JAC_EVENT_TEPPO_DENRAI', 'SENGOKU', 1543, 'TECH_JAC_SENGOKU_FIREARMS_CONTACT', NULL,
		'LOC_JAC_EVENT_TEPPO_DENRAI_NAME', 'LOC_JAC_EVENT_TEPPO_DENRAI_DESCRIPTION', 0),
	('JAC_EVENT_ASHIO_COPPER', 'SENGOKU', 1550, 'TECH_JAC_SENGOKU_MINE_MANAGEMENT', NULL,
		'LOC_JAC_EVENT_ASHIO_COPPER_NAME', 'LOC_JAC_EVENT_ASHIO_COPPER_DESCRIPTION', 0),
	('JAC_EVENT_NAGAO_UESUGI', 'SENGOKU', 1561, NULL, 'CIVIC_JAC_SENGOKU_COURT_AND_SHOGUNATE_ENVOYS',
		'LOC_JAC_EVENT_NAGAO_UESUGI_NAME', 'LOC_JAC_EVENT_NAGAO_UESUGI_DESCRIPTION', 1);

INSERT INTO JAC_HistoricalEventTargets (EventType, CivilizationType) VALUES
	('JAC_EVENT_NAGAO_UESUGI', 'CIVILIZATION_JAC_NAGAO');

INSERT INTO JAC_HistoricalEventRenames (EventType, CivilizationType, Name) VALUES
	('JAC_EVENT_NAGAO_UESUGI', 'CIVILIZATION_JAC_NAGAO', 'LOC_JAC_EVENT_UESUGI_CIV_NAME');

-- Owner decision 1a: when the year arrives, a faction without the related tech/civic gets its boost
-- (eureka/inspiration) through PlayerTechs/PlayerCulture:TriggerBoost, which needs a Boosts row.
-- Placeholder rows (暫定, the real boosts belong to the tech/civic tree); skipped if the tree defines one.
INSERT INTO Boosts (TechnologyType, Boost, TriggerDescription, TriggerLongDescription, BoostClass, Unit1Type)
SELECT t.TechnologyType, 40, 'LOC_JAC_EVENT_BOOST_PLACEHOLDER', 'LOC_JAC_EVENT_BOOST_PLACEHOLDER',
	'BOOST_TRIGGER_TRAIN_UNIT', 'UNIT_GALLEY'
FROM Technologies t
WHERE t.TechnologyType IN ('TECH_JAC_SENGOKU_FIREARMS_CONTACT', 'TECH_JAC_SENGOKU_MINE_MANAGEMENT')
	AND NOT EXISTS (SELECT 1 FROM Boosts b WHERE b.TechnologyType = t.TechnologyType);

INSERT INTO Boosts (CivicType, Boost, TriggerDescription, TriggerLongDescription, BoostClass, Unit1Type)
SELECT c.CivicType, 40, 'LOC_JAC_EVENT_BOOST_PLACEHOLDER', 'LOC_JAC_EVENT_BOOST_PLACEHOLDER',
	'BOOST_TRIGGER_TRAIN_UNIT', 'UNIT_GALLEY'
FROM Civics c
WHERE c.CivicType = 'CIVIC_JAC_SENGOKU_COURT_AND_SHOGUNATE_ENVOYS'
	AND NOT EXISTS (SELECT 1 FROM Boosts b WHERE b.CivicType = c.CivicType);
