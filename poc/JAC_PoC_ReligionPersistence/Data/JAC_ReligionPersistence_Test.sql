-- Loaded only with the driver's explicit --test-mode configuration.
-- Shorten only our revival cooldown. SetHasAllied still creates a 30-turn
-- engine alliance even when the SQL global parameter is changed to one.
-- The scenario uses test-mode fast-forward and real turn processing for expiry.
UPDATE JAC_RP_Rules SET Value = 1 WHERE ID = 'REVIVAL_COOLDOWN_TURNS';
