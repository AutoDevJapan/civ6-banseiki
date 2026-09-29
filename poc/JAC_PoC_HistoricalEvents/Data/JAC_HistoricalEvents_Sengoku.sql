-- Sengoku pack sample events. Values are provisional (暫定); see documents/poc/historical-events.md.

-- Year -> turn: turn = (Year - START_YEAR) * TURNS_PER_YEAR + 1 (standard speed, ~200 turns for 1534-1600).
-- OR IGNORE so the calendar implementation can own these rows once it exists.
INSERT OR IGNORE INTO GlobalParameters (Name, Value) VALUES
	('JAC_EVENTS_START_YEAR', '1534'),
	('JAC_EVENTS_TURNS_PER_YEAR', '3');

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

-- At the historical year the firearms knowledge reaches every faction that has not researched it.
-- Same modifier type and argument as the official Maori trait (Expansion2_Civilizations_Major.xml:479-481, 973-975).
INSERT INTO Modifiers (ModifierId, ModifierType) VALUES
	('JAC_EVENT_TEPPO_DENRAI_GRANT_TECH', 'MODIFIER_PLAYER_GRANT_SPECIFIC_TECHNOLOGY');
INSERT INTO ModifierArguments (ModifierId, Name, Value) VALUES
	('JAC_EVENT_TEPPO_DENRAI_GRANT_TECH', 'TechType', 'TECH_JAC_SENGOKU_FIREARMS_CONTACT');
INSERT INTO JAC_HistoricalEventModifiers (EventType, ModifierId) VALUES
	('JAC_EVENT_TEPPO_DENRAI', 'JAC_EVENT_TEPPO_DENRAI_GRANT_TECH');
