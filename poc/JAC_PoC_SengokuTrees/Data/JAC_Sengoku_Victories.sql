-- Official victory types remain in Types for GS references, but cannot end a
-- Sengoku game before the separate castle-unification victory is implemented.
UPDATE Victories SET EnabledByDefault = 0
WHERE VictoryType IN (
    'VICTORY_DEFAULT', 'VICTORY_CONQUEST', 'VICTORY_TECHNOLOGY',
    'VICTORY_CULTURE', 'VICTORY_RELIGIOUS', 'VICTORY_DIPLOMATIC',
    'VICTORY_SCORE'
);

UPDATE Defeats SET EnabledByDefault = 0 WHERE DefeatType = 'DEFEAT_TIME';
