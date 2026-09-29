-- Historical events (SSOT 3.2). One row per event; effects live in child tables so a
-- researched event list can be loaded as data only (see documents/poc/historical-events.md).

CREATE TABLE JAC_HistoricalEvents (
	EventType TEXT NOT NULL PRIMARY KEY,
	Pack TEXT NOT NULL,
	-- Historical year. Converted to a turn with JAC_EVENTS_START_YEAR / JAC_EVENTS_TURNS_PER_YEAR.
	Year INTEGER,
	-- Explicit turn; wins over Year when both are set.
	Turn INTEGER,
	-- A target faction that already has this tech or civic fires the event before its turn.
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

-- Resources that appear on the map the first time the event fires for anyone.
-- PlotIndex is map-specific, so rows are loaded per map file (modinfo criteria).
CREATE TABLE JAC_HistoricalEventResources (
	EventType TEXT NOT NULL REFERENCES JAC_HistoricalEvents(EventType) ON DELETE CASCADE,
	ResourceType TEXT NOT NULL REFERENCES Resources(ResourceType),
	PlotIndex INTEGER NOT NULL,
	-- The nearest legal plot within this range is used (ResourceBuilder.CanHaveResource).
	SearchRange INTEGER NOT NULL DEFAULT 2,
	Amount INTEGER NOT NULL DEFAULT 1,
	SiteId TEXT,
	PRIMARY KEY (EventType, ResourceType, PlotIndex)
);
