-- Loaded only with the driver's explicit --test-mode configuration.
-- Keep real engine expiry and cooldown checks; shorten their waiting periods.
UPDATE JAC_RP_Rules SET Value = 1 WHERE ID = 'REVIVAL_COOLDOWN_TURNS';
UPDATE GlobalParameters SET Value = 1 WHERE Name = 'DIPLOMACY_ALLIANCE_TIME_LIMIT';
