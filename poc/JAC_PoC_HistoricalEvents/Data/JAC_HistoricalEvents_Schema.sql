-- Historical events (SSOT 3.2). One row per event; effects live in child tables so a
-- researched event list can be loaded as data only (see documents/poc/historical-events.md).

CREATE TABLE JAC_HistoricalEvents (
	EventType TEXT NOT NULL PRIMARY KEY,
	Pack TEXT NOT NULL,
	-- Historical year. Converted to a turn by the Lua script from the game speed's calendar (GameSpeed_Turns).
	Year INTEGER,
	-- Month of the year (1-12) as the sources give it (SSOT 3.2: match the month as far as possible). NULL: month 1.
	Month INTEGER CHECK (Month BETWEEN 1 AND 12),
	-- Explicit turn; wins over Year when both are set.
	Turn INTEGER,
	-- A target faction that has this tech or civic gets the event's effects (before its turn if it researched
	-- first); one that has not gets only a boost toward it (not the tech itself) when the year arrives.
	PrereqTech TEXT REFERENCES Technologies(TechnologyType),
	PrereqCivic TEXT REFERENCES Civics(CivicType),
	Name TEXT NOT NULL,
	Description TEXT NOT NULL,
	-- 0: notify the faction the event fired for. 1: notify every alive major faction.
	NotifyAll BOOLEAN NOT NULL DEFAULT 0,
	CHECK (Year IS NOT NULL OR Turn IS NOT NULL)
);

-- No rows: every alive major faction is a target.
CREATE TABLE JAC_HistoricalEventTargets (
	EventType TEXT NOT NULL REFERENCES JAC_HistoricalEvents(EventType) ON DELETE CASCADE,
	CivilizationType TEXT NOT NULL,
	PRIMARY KEY (EventType, CivilizationType)
);

-- Player modifiers attached once to the faction the event fires for.
CREATE TABLE JAC_HistoricalEventModifiers (
	EventType TEXT NOT NULL REFERENCES JAC_HistoricalEvents(EventType) ON DELETE CASCADE,
	ModifierId TEXT NOT NULL REFERENCES Modifiers(ModifierId),
	PRIMARY KEY (EventType, ModifierId)
);

-- Display-name switch (SSOT 3.1). The civilization ID never changes; UI reads the shared name.
CREATE TABLE JAC_HistoricalEventRenames (
	EventType TEXT NOT NULL REFERENCES JAC_HistoricalEvents(EventType) ON DELETE CASCADE,
	CivilizationType TEXT NOT NULL,
	Name TEXT NOT NULL,
	PRIMARY KEY (EventType, CivilizationType)
);

-- Reserved resource sites revealed the first time the event happens (fires or its year arrives) for anyone.
-- SiteId is a site of the Japan maps' resources mod (JAC_PoC_JapanMapScale Scripts/JAC_ResourceSites.lua);
-- the site knows its tile on each map, so one row serves every map.
CREATE TABLE JAC_HistoricalEventResources (
	EventType TEXT NOT NULL REFERENCES JAC_HistoricalEvents(EventType) ON DELETE CASCADE,
	SiteId TEXT NOT NULL,
	PRIMARY KEY (EventType, SiteId)
);
