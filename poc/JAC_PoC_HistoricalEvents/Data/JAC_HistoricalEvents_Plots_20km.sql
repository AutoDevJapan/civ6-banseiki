-- 20 km Japan maps (historical and settler variants share the grid). PlotIndex is the tile nearest to the
-- site in documents/spec/data/japan-map-resources.json, projected with Maps/manifest.json (checked by tests).
INSERT INTO JAC_HistoricalEventResources (EventType, ResourceType, PlotIndex, SearchRange, Amount, SiteId) VALUES
	('JAC_EVENT_ASHIO_COPPER', 'RESOURCE_COPPER', 4416, 2, 1, 'copper-ashio');
