-- Sengoku ages (SSOT 3.2, documents/research/sengoku-ages.md): the official era score, ages and dedications
-- are switched off and Scripts/JAC_Sengoku_Ages.lua (the "武功の台帳") scores and judges each house at every
-- 改元. All numbers are provisional (暫定). Runs in the pack's UpdateDatabase (LoadOrder 200), after the
-- official data and after the Dramatic Ages mode (Byzantium_Gaul.modinfo:99-104, no LoadOrder = 0), so the
-- values below are the final ones (design §4.2).

-- §4.2: no official age can happen. The official judgement compares the era's points E with base + shifts
-- (DLC/Expansion2/UI/Additions/EraProgressPanel.lua:117-133); with every shift 0 and the bases far apart,
-- dark <= 0 <= E < golden always holds. Official values: Expansion2_GlobalParameters.xml:62-66,126,248-264.
INSERT OR REPLACE INTO GlobalParameters (Name, Value) VALUES
  ('DARK_AGE_SCORE_BASE_THRESHOLD', '-9999'),
  ('GOLDEN_AGE_SCORE_BASE_THRESHOLD', '9999'),
  ('THRESHOLD_SHIFT_PER_CITY', '0'),
  ('THRESHOLD_SHIFT_PER_PAST_GOLDEN_AGE', '0'),
  ('THRESHOLD_SHIFT_PER_PAST_DARK_AGE', '0'),
  ('THRESHOLD_SHIFT_PER_ANARCHY', '0'),
  ('THRESHOLD_SHIFT_PER_INCOMPLETE_ERA_CIVIC', '0'),
  ('THRESHOLD_SHIFT_PER_INCOMPLETE_ERA_TECH', '0'),
  ('THRESHOLD_SHIFT_PER_INCOMPLETE_OLD_CIVIC', '0'),
  ('THRESHOLD_SHIFT_PER_INCOMPLETE_OLD_TECH', '0'),
  ('THRESHOLD_SHIFT_PER_MISSING_AMENITY', '0'),
  -- No official dedication dialog (as the Dramatic Ages mode: Byzantium_Gaul_DramaticAges_MODE.xml:6-18).
  ('COMMEMORATE_BASE_CHOICES_ALLOWED', '0'),
  ('COMMEMORATE_OPTIONS_MAX', '0');

-- ERA_ANCIENT -3 (DLC/Expansion2/Data/Expansion2_Eras.xml:65).
UPDATE Eras_XP2 SET EraScoreThresholdShift = 0;

-- Historic moments stay in the timeline without points (Expansion1_Moments.xml:191-321).
UPDATE Moments SET EraScore = 0;

-- The policy reopen entry must survive turn phases and save/load. Ordinary reign notices use USER_DEFINED_2.
-- Official precedent: DLC/PiratesScenario/Data/PiratesScenario_Notifications.xml:7-10 makes USER notifications
-- durable; Base/Assets/Gameplay/Data/Notifications.xml:166-167 otherwise expires both at turn end.
UPDATE Notifications SET ExpiresEndOfTurn = 0 WHERE NotificationType = 'NOTIFICATION_USER_DEFINED_3';

-- Every attachment of a modifier whose type adds era score (types: Expansion1_Modifiers.xml:15-32,184-186,
-- Expansion2_Modifiers.xml:80). Official attachments: dedication quests (CommemorationModifiers), the Taj Mahal
-- (DLC/Expansion1/Data/Expansion1_Buildings_Major.xml:78) and the Dramatic Ages GameModifiers
-- (Byzantium_Gaul_DramaticAges_MODE.xml:96-100). These tables are every official table with a ModifierId
-- column; the tables do not reference Modifiers, so deleting the Modifiers rows would leave dangling IDs.
DELETE FROM AllianceEffects WHERE ModifierID IN (SELECT ModifierId FROM Modifiers WHERE ModifierType LIKE '%ERA_SCORE%');
DELETE FROM BeliefModifiers WHERE ModifierID IN (SELECT ModifierId FROM Modifiers WHERE ModifierType LIKE '%ERA_SCORE%');
DELETE FROM BuildingModifiers WHERE ModifierId IN (SELECT ModifierId FROM Modifiers WHERE ModifierType LIKE '%ERA_SCORE%');
DELETE FROM CivicModifiers WHERE ModifierId IN (SELECT ModifierId FROM Modifiers WHERE ModifierType LIKE '%ERA_SCORE%');
DELETE FROM CommemorationModifiers WHERE ModifierId IN (SELECT ModifierId FROM Modifiers WHERE ModifierType LIKE '%ERA_SCORE%');
DELETE FROM ComplimentModifiers WHERE ModifierId IN (SELECT ModifierId FROM Modifiers WHERE ModifierType LIKE '%ERA_SCORE%');
DELETE FROM DistrictModifiers WHERE ModifierId IN (SELECT ModifierId FROM Modifiers WHERE ModifierType LIKE '%ERA_SCORE%');
DELETE FROM EmergencyBuffs WHERE ModifierID IN (SELECT ModifierId FROM Modifiers WHERE ModifierType LIKE '%ERA_SCORE%');
DELETE FROM EmergencyRewards WHERE ModifierID IN (SELECT ModifierId FROM Modifiers WHERE ModifierType LIKE '%ERA_SCORE%');
DELETE FROM GameModifiers WHERE ModifierId IN (SELECT ModifierId FROM Modifiers WHERE ModifierType LIKE '%ERA_SCORE%');
DELETE FROM GoodyHutSubTypes WHERE ModifierID IN (SELECT ModifierId FROM Modifiers WHERE ModifierType LIKE '%ERA_SCORE%');
DELETE FROM GovernmentModifiers WHERE ModifierId IN (SELECT ModifierId FROM Modifiers WHERE ModifierType LIKE '%ERA_SCORE%');
DELETE FROM GovernorModifiers WHERE ModifierId IN (SELECT ModifierId FROM Modifiers WHERE ModifierType LIKE '%ERA_SCORE%');
DELETE FROM GovernorPromotionModifiers WHERE ModifierId IN (SELECT ModifierId FROM Modifiers WHERE ModifierType LIKE '%ERA_SCORE%');
DELETE FROM GreatPersonIndividualActionModifiers WHERE ModifierId IN (SELECT ModifierId FROM Modifiers WHERE ModifierType LIKE '%ERA_SCORE%');
DELETE FROM GreatPersonIndividualBirthModifiers WHERE ModifierId IN (SELECT ModifierId FROM Modifiers WHERE ModifierType LIKE '%ERA_SCORE%');
DELETE FROM GreatWorkModifiers WHERE ModifierId IN (SELECT ModifierId FROM Modifiers WHERE ModifierType LIKE '%ERA_SCORE%');
DELETE FROM ImprovementModifiers WHERE ModifierId IN (SELECT ModifierId FROM Modifiers WHERE ModifierType LIKE '%ERA_SCORE%');
DELETE FROM PolicyModifiers WHERE ModifierId IN (SELECT ModifierId FROM Modifiers WHERE ModifierType LIKE '%ERA_SCORE%');
DELETE FROM ProjectCompletionModifiers WHERE ModifierId IN (SELECT ModifierId FROM Modifiers WHERE ModifierType LIKE '%ERA_SCORE%');
DELETE FROM ResolutionEffects WHERE ModifierId IN (SELECT ModifierId FROM Modifiers WHERE ModifierType LIKE '%ERA_SCORE%');
DELETE FROM TechnologyModifiers WHERE ModifierId IN (SELECT ModifierId FROM Modifiers WHERE ModifierType LIKE '%ERA_SCORE%');
DELETE FROM TraitModifiers WHERE ModifierId IN (SELECT ModifierId FROM Modifiers WHERE ModifierType LIKE '%ERA_SCORE%');
DELETE FROM UnitAbilityModifiers WHERE ModifierId IN (SELECT ModifierId FROM Modifiers WHERE ModifierType LIKE '%ERA_SCORE%');
DELETE FROM UnitPromotionModifiers WHERE ModifierId IN (SELECT ModifierId FROM Modifiers WHERE ModifierType LIKE '%ERA_SCORE%');

-- Effects tied to the official ages (in case one happens anyway): the loyalty +-0.5 per citizen
-- (DLC/Expansion2/Data/Expansion1_Leaders.xml:138-139) and the golden-age war (Expansion1_DiplomaticActions.xml:5).
DELETE FROM TraitModifiers WHERE TraitType = 'TRAIT_LEADER_MAJOR_CIV'
  AND ModifierId IN ('GOLDEN_AGE_CITY_IDENTITY', 'DARK_AGE_CITY_IDENTITY');
DELETE FROM DiplomaticActions WHERE DiplomaticActionType = 'DIPLOACTION_DECLARE_GOLDEN_AGE_WAR';

-- §4.1: the 改元 after 天文 (the start). The rows are Data/JAC_Sengoku_Reigns.sql, generated by
-- tools/generate_sengoku_reigns.py from documents/spec/data/sengoku-reigns.json: the historical era names from
-- 弘治 (1555) to 令和 (2019), then 遅代 (2119, 100 years of 令和) with ForcedAge DARK (SSOT 3.2). Western dates;
-- the ledger judges when the reign's engine era starts (元号 = 時代, owner decision T: each reign is one era of
-- Data/JAC_Sengoku_Eras.sql, lasting its reign's turns; documents/poc/era-equals-reign.md).
CREATE TABLE JAC_SengokuReigns (
  ReignType TEXT NOT NULL PRIMARY KEY,
  Year INTEGER NOT NULL,
  Month INTEGER NOT NULL CHECK (Month BETWEEN 1 AND 12),
  Name TEXT NOT NULL,
  -- Title of the 改元 screen ("改元", "天文から弘治へ（1555年）").
  Announcement TEXT NOT NULL,
  -- Heading of the 改元 notification ("改元：弘治の世").
  Notification TEXT NOT NULL,
  -- Age of every house at this 改元 whatever its score (遅代: DARK); NULL is the ordinary judgement.
  ForcedAge TEXT CHECK (ForcedAge IS NULL OR ForcedAge = 'DARK'),
  -- The engine era of this reign (天文, the start, is the first era).
  EraType TEXT NOT NULL UNIQUE REFERENCES Eras (EraType)
);

-- §5.1: points of a historical event for the house it fired for (JAC_PoC_HistoricalEvents writes the player
-- property JAC_EVENT_FIRED_<EventType>). No reference: that mod is optional and loads after this one.
CREATE TABLE JAC_SengokuAgeEventScores (
  EventType TEXT NOT NULL PRIMARY KEY,
  Score INTEGER NOT NULL CHECK (Score > 0)
);
INSERT INTO JAC_SengokuAgeEventScores (EventType, Score) VALUES
  ('JAC_EVENT_TEPPO', 2),
  ('JAC_EVENT_IKUNO_SILVER', 1),
  ('JAC_EVENT_NAGAO_TO_UESUGI', 3);

-- §7.2: dark-age policies. A house judged dark gets its default at once (its row of
-- JAC_SengokuDarkAgePolicyDefaults, else the lowest Priority); a human house may choose another one on the choice
-- screen (UI/JAC_SengokuDarkPolicy.lua), once per 改元. Priority is also the screen's order. A city-wide effect is
-- on while the city centre's plot property <PolicyType> is 1; a house-wide effect while JAC_AGE_ANCHOR_<suffix>
-- is 1 on the house's anchor city (one city per house, §7.1).
CREATE TABLE JAC_SengokuDarkAgePolicies (
  PolicyType TEXT NOT NULL PRIMARY KEY CHECK (PolicyType LIKE 'JAC_AGE_POLICY_%'),
  Priority INTEGER NOT NULL UNIQUE,
  Name TEXT NOT NULL,
  Description TEXT NOT NULL,
  Announcement TEXT NOT NULL
);
INSERT INTO JAC_SengokuDarkAgePolicies (PolicyType, Priority, Name, Description, Announcement) VALUES
  ('JAC_AGE_POLICY_VALOR', 1, 'LOC_JAC_AGE_POLICY_VALOR_NAME', 'LOC_JAC_AGE_POLICY_VALOR_DESCRIPTION', 'LOC_JAC_AGE_DARK_VALOR_ANNOUNCEMENT'),
  ('JAC_AGE_POLICY_ELITE', 2, 'LOC_JAC_AGE_POLICY_ELITE_NAME', 'LOC_JAC_AGE_POLICY_ELITE_DESCRIPTION', 'LOC_JAC_AGE_DARK_ELITE_ANNOUNCEMENT'),
  ('JAC_AGE_POLICY_PIRATE', 3, 'LOC_JAC_AGE_POLICY_PIRATE_NAME', 'LOC_JAC_AGE_POLICY_PIRATE_DESCRIPTION', 'LOC_JAC_AGE_DARK_PIRATE_ANNOUNCEMENT'),
  ('JAC_AGE_POLICY_TEMPLE', 4, 'LOC_JAC_AGE_POLICY_TEMPLE_NAME', 'LOC_JAC_AGE_POLICY_TEMPLE_DESCRIPTION', 'LOC_JAC_AGE_DARK_TEMPLE_ANNOUNCEMENT');

-- §6.2 / §7.2: each house's dark-age policy when it has not chosen (AI houses always; humans until they choose).
-- The design's per-house ranking reduced to its first place: every policy can always be taken, so a lower place
-- is never used. Owner decision (2026-09-30): keep this assignment and tune it after ~200 turns. After the house's historical profile: Takeda / Oda / Nagao /
-- Shimazu famed armies -> ELITE; sea powers (Mori's Murakami, Chosokabe, Otomo, Ryukyu) -> PIRATE; temple and
-- court culture (Honganji, Ashikaga, Imagawa, Asakura, Ouchi) -> TEMPLE; the rest -> VALOR.
-- CivilizationType has no foreign key: the houses come from JAC_PoC_CustomCivs, and other maps' civilizations
-- simply have no row.
CREATE TABLE JAC_SengokuDarkAgePolicyDefaults (
  CivilizationType TEXT NOT NULL PRIMARY KEY,
  PolicyType TEXT NOT NULL REFERENCES JAC_SengokuDarkAgePolicies (PolicyType)
);
INSERT INTO JAC_SengokuDarkAgePolicyDefaults (CivilizationType, PolicyType) VALUES
  ('CIVILIZATION_JAC_AINU', 'JAC_AGE_POLICY_VALOR'),
  ('CIVILIZATION_JAC_AMAGO', 'JAC_AGE_POLICY_VALOR'),
  ('CIVILIZATION_JAC_ASAKURA', 'JAC_AGE_POLICY_TEMPLE'),
  ('CIVILIZATION_JAC_ASHIKAGA', 'JAC_AGE_POLICY_TEMPLE'),
  ('CIVILIZATION_JAC_ASHINA', 'JAC_AGE_POLICY_VALOR'),
  ('CIVILIZATION_JAC_CHOSOKABE', 'JAC_AGE_POLICY_PIRATE'),
  ('CIVILIZATION_JAC_DATE', 'JAC_AGE_POLICY_VALOR'),
  ('CIVILIZATION_JAC_HOJO', 'JAC_AGE_POLICY_VALOR'),
  ('CIVILIZATION_JAC_HONGANJI', 'JAC_AGE_POLICY_TEMPLE'),
  ('CIVILIZATION_JAC_IMAGAWA', 'JAC_AGE_POLICY_TEMPLE'),
  ('CIVILIZATION_JAC_MATSUDAIRA', 'JAC_AGE_POLICY_VALOR'),
  ('CIVILIZATION_JAC_MIYOSHI', 'JAC_AGE_POLICY_VALOR'),
  ('CIVILIZATION_JAC_MORI', 'JAC_AGE_POLICY_PIRATE'),
  ('CIVILIZATION_JAC_NAGAO', 'JAC_AGE_POLICY_ELITE'),
  ('CIVILIZATION_JAC_ODA', 'JAC_AGE_POLICY_ELITE'),
  ('CIVILIZATION_JAC_OTOMO', 'JAC_AGE_POLICY_PIRATE'),
  ('CIVILIZATION_JAC_OUCHI', 'JAC_AGE_POLICY_TEMPLE'),
  ('CIVILIZATION_JAC_ROKKAKU', 'JAC_AGE_POLICY_VALOR'),
  ('CIVILIZATION_JAC_RYUKYU', 'JAC_AGE_POLICY_PIRATE'),
  ('CIVILIZATION_JAC_SHIMAZU', 'JAC_AGE_POLICY_ELITE'),
  ('CIVILIZATION_JAC_TAKEDA', 'JAC_AGE_POLICY_ELITE'),
  ('CIVILIZATION_JAC_YAMANOUCHI_UESUGI', 'JAC_AGE_POLICY_VALOR');

-- §7.1: conditions read the city centre's plot property. REQUIREMENT_PLOT_PROPERTY_MATCHES is the only
-- official property requirement; its only official use gives PropertyName and PropertyMinimum
-- (DLC/CivRoyaleScenario/Data/CivRoyaleScenario_Civilizations.xml:377-403), so every state is its own 0/1 flag.
INSERT INTO Requirements (RequirementId, RequirementType) VALUES
  ('JAC_AGE_REQ_GOLDEN', 'REQUIREMENT_PLOT_PROPERTY_MATCHES'),
  ('JAC_AGE_REQ_POLICY_PIRATE', 'REQUIREMENT_PLOT_PROPERTY_MATCHES'),
  ('JAC_AGE_REQ_POLICY_TEMPLE', 'REQUIREMENT_PLOT_PROPERTY_MATCHES'),
  ('JAC_AGE_REQ_ANCHOR_VALOR', 'REQUIREMENT_PLOT_PROPERTY_MATCHES'),
  ('JAC_AGE_REQ_ANCHOR_ELITE', 'REQUIREMENT_PLOT_PROPERTY_MATCHES'),
  ('JAC_AGE_REQ_ANCHOR_PIRATE', 'REQUIREMENT_PLOT_PROPERTY_MATCHES');
INSERT INTO RequirementArguments (RequirementId, Name, Value) VALUES
  ('JAC_AGE_REQ_GOLDEN', 'PropertyName', 'JAC_AGE_GOLDEN'),
  ('JAC_AGE_REQ_GOLDEN', 'PropertyMinimum', '1'),
  ('JAC_AGE_REQ_POLICY_PIRATE', 'PropertyName', 'JAC_AGE_POLICY_PIRATE'),
  ('JAC_AGE_REQ_POLICY_PIRATE', 'PropertyMinimum', '1'),
  ('JAC_AGE_REQ_POLICY_TEMPLE', 'PropertyName', 'JAC_AGE_POLICY_TEMPLE'),
  ('JAC_AGE_REQ_POLICY_TEMPLE', 'PropertyMinimum', '1'),
  ('JAC_AGE_REQ_ANCHOR_VALOR', 'PropertyName', 'JAC_AGE_ANCHOR_VALOR'),
  ('JAC_AGE_REQ_ANCHOR_VALOR', 'PropertyMinimum', '1'),
  ('JAC_AGE_REQ_ANCHOR_ELITE', 'PropertyName', 'JAC_AGE_ANCHOR_ELITE'),
  ('JAC_AGE_REQ_ANCHOR_ELITE', 'PropertyMinimum', '1'),
  ('JAC_AGE_REQ_ANCHOR_PIRATE', 'PropertyName', 'JAC_AGE_ANCHOR_PIRATE'),
  ('JAC_AGE_REQ_ANCHOR_PIRATE', 'PropertyMinimum', '1');
INSERT INTO RequirementSets (RequirementSetId, RequirementSetType) VALUES
  ('JAC_AGE_CITY_GOLDEN', 'REQUIREMENTSET_TEST_ALL'),
  ('JAC_AGE_CITY_PIRATE', 'REQUIREMENTSET_TEST_ALL'),
  ('JAC_AGE_CITY_TEMPLE', 'REQUIREMENTSET_TEST_ALL'),
  ('JAC_AGE_ANCHOR_VALOR', 'REQUIREMENTSET_TEST_ALL'),
  ('JAC_AGE_ANCHOR_ELITE', 'REQUIREMENTSET_TEST_ALL'),
  ('JAC_AGE_ANCHOR_PIRATE', 'REQUIREMENTSET_TEST_ALL'),
  ('JAC_AGE_UNIT_IS_SEA', 'REQUIREMENTSET_TEST_ALL');
INSERT INTO RequirementSetRequirements (RequirementSetId, RequirementId) VALUES
  ('JAC_AGE_CITY_GOLDEN', 'JAC_AGE_REQ_GOLDEN'),
  ('JAC_AGE_CITY_PIRATE', 'JAC_AGE_REQ_POLICY_PIRATE'),
  ('JAC_AGE_CITY_TEMPLE', 'JAC_AGE_REQ_POLICY_TEMPLE'),
  ('JAC_AGE_ANCHOR_VALOR', 'JAC_AGE_REQ_ANCHOR_VALOR'),
  ('JAC_AGE_ANCHOR_ELITE', 'JAC_AGE_REQ_ANCHOR_ELITE'),
  ('JAC_AGE_ANCHOR_PIRATE', 'JAC_AGE_REQ_ANCHOR_PIRATE'),
  ('JAC_AGE_UNIT_IS_SEA', 'REQUIRES_SEA_DOMAIN');

-- Effects (§7.1-7.2). City effects use the city type of MONASTICISM_CULTURE_MODIFIER
-- (Expansion1_Policies.xml:279-282) gated by the Subject requirement. House effects copy the official dark
-- policies' types and values (TWILIGHTVALOR_*, ELITEFORCES_*, LETTEROFMARQUE_*: Expansion1_Policies.xml:283-285,
-- 354-357,427-432, 1617-1627,1657-1761) and are attached to every city with MODIFIER_PLAYER_CITIES_ATTACH_MODIFIER
-- (Base/Assets/Gameplay/Data/Modifiers.xml:780); the attached modifier's OwnerRequirementSetId keeps it on only
-- in the anchor city. Simplifications (provisional): naval production uses the Cothon's domain type
-- (MODIFIER_CITY_ADJUST_UNIT_DOMAIN_PRODUCTION, Amount 100) instead of the per-era raider rows, and the trade
-- penalty is gold only.
INSERT INTO Modifiers (ModifierId, ModifierType, OwnerRequirementSetId, SubjectRequirementSetId) VALUES
  ('JAC_AGE_GOLDEN_PRODUCTION', 'MODIFIER_PLAYER_CITIES_ADJUST_CITY_YIELD_MODIFIER', NULL, 'JAC_AGE_CITY_GOLDEN'),
  ('JAC_AGE_GOLDEN_GOLD', 'MODIFIER_PLAYER_CITIES_ADJUST_CITY_YIELD_MODIFIER', NULL, 'JAC_AGE_CITY_GOLDEN'),
  ('JAC_AGE_TEMPLE_SCIENCE', 'MODIFIER_PLAYER_CITIES_ADJUST_CITY_YIELD_MODIFIER', NULL, 'JAC_AGE_CITY_TEMPLE'),
  ('JAC_AGE_TEMPLE_CULTURE', 'MODIFIER_PLAYER_CITIES_ADJUST_CITY_YIELD_MODIFIER', NULL, 'JAC_AGE_CITY_TEMPLE'),
  ('JAC_AGE_ATTACH_PIRATE_PRODUCTION', 'MODIFIER_PLAYER_CITIES_ATTACH_MODIFIER', NULL, NULL),
  ('JAC_AGE_PIRATE_PRODUCTION', 'MODIFIER_CITY_ADJUST_UNIT_DOMAIN_PRODUCTION', 'JAC_AGE_CITY_PIRATE', NULL),
  ('JAC_AGE_ATTACH_PIRATE_MOVEMENT', 'MODIFIER_PLAYER_CITIES_ATTACH_MODIFIER', NULL, NULL),
  ('JAC_AGE_PIRATE_MOVEMENT', 'MODIFIER_PLAYER_UNITS_ADJUST_MOVEMENT', 'JAC_AGE_ANCHOR_PIRATE', 'JAC_AGE_UNIT_IS_SEA'),
  ('JAC_AGE_ATTACH_PIRATE_TRADE', 'MODIFIER_PLAYER_CITIES_ATTACH_MODIFIER', NULL, NULL),
  ('JAC_AGE_PIRATE_TRADE', 'MODIFIER_PLAYER_ADJUST_TRADE_ROUTE_YIELD_MODIFIER', 'JAC_AGE_ANCHOR_PIRATE', NULL),
  ('JAC_AGE_ATTACH_VALOR_ATTACK', 'MODIFIER_PLAYER_CITIES_ATTACH_MODIFIER', NULL, NULL),
  ('JAC_AGE_VALOR_ATTACK', 'MODIFIER_PLAYER_UNITS_GRANT_ABILITY', 'JAC_AGE_ANCHOR_VALOR', NULL),
  ('JAC_AGE_ATTACH_VALOR_NO_FOREIGN_HEAL', 'MODIFIER_PLAYER_CITIES_ATTACH_MODIFIER', NULL, NULL),
  ('JAC_AGE_VALOR_NO_FOREIGN_HEAL', 'MODIFIER_PLAYER_DISABLE_HEALING', 'JAC_AGE_ANCHOR_VALOR', NULL),
  ('JAC_AGE_ATTACH_VALOR_NO_NEUTRAL_HEAL', 'MODIFIER_PLAYER_CITIES_ATTACH_MODIFIER', NULL, NULL),
  ('JAC_AGE_VALOR_NO_NEUTRAL_HEAL', 'MODIFIER_PLAYER_DISABLE_HEALING', 'JAC_AGE_ANCHOR_VALOR', NULL),
  ('JAC_AGE_ATTACH_ELITE_EXPERIENCE', 'MODIFIER_PLAYER_CITIES_ATTACH_MODIFIER', NULL, NULL),
  ('JAC_AGE_ELITE_EXPERIENCE', 'MODIFIER_PLAYER_UNITS_ADJUST_UNIT_EXPERIENCE_MODIFIER', 'JAC_AGE_ANCHOR_ELITE', NULL),
  ('JAC_AGE_ATTACH_ELITE_MAINTENANCE', 'MODIFIER_PLAYER_CITIES_ATTACH_MODIFIER', NULL, NULL),
  ('JAC_AGE_ELITE_MAINTENANCE', 'MODIFIER_PLAYER_ADJUST_UNIT_MAINTENANCE_DISCOUNT', 'JAC_AGE_ANCHOR_ELITE', NULL);
INSERT INTO ModifierArguments (ModifierId, Name, Value) VALUES
  ('JAC_AGE_GOLDEN_PRODUCTION', 'YieldType', 'YIELD_PRODUCTION'),
  ('JAC_AGE_GOLDEN_PRODUCTION', 'Amount', '10'),
  ('JAC_AGE_GOLDEN_GOLD', 'YieldType', 'YIELD_GOLD'),
  ('JAC_AGE_GOLDEN_GOLD', 'Amount', '10'),
  ('JAC_AGE_TEMPLE_SCIENCE', 'YieldType', 'YIELD_SCIENCE'),
  ('JAC_AGE_TEMPLE_SCIENCE', 'Amount', '25'),
  ('JAC_AGE_TEMPLE_CULTURE', 'YieldType', 'YIELD_CULTURE'),
  ('JAC_AGE_TEMPLE_CULTURE', 'Amount', '-25'),
  ('JAC_AGE_ATTACH_PIRATE_PRODUCTION', 'ModifierId', 'JAC_AGE_PIRATE_PRODUCTION'),
  ('JAC_AGE_PIRATE_PRODUCTION', 'Domain', 'DOMAIN_SEA'),
  ('JAC_AGE_PIRATE_PRODUCTION', 'Amount', '100'),
  ('JAC_AGE_ATTACH_PIRATE_MOVEMENT', 'ModifierId', 'JAC_AGE_PIRATE_MOVEMENT'),
  ('JAC_AGE_PIRATE_MOVEMENT', 'Amount', '2'),
  ('JAC_AGE_ATTACH_PIRATE_TRADE', 'ModifierId', 'JAC_AGE_PIRATE_TRADE'),
  ('JAC_AGE_PIRATE_TRADE', 'YieldType', 'YIELD_GOLD'),
  ('JAC_AGE_PIRATE_TRADE', 'Origin', '1'),
  ('JAC_AGE_PIRATE_TRADE', 'Amount', '-50'),
  ('JAC_AGE_ATTACH_VALOR_ATTACK', 'ModifierId', 'JAC_AGE_VALOR_ATTACK'),
  ('JAC_AGE_VALOR_ATTACK', 'AbilityType', 'ABILITY_TWILIGHT_VALOR_ATTACK_BONUS'),
  ('JAC_AGE_ATTACH_VALOR_NO_FOREIGN_HEAL', 'ModifierId', 'JAC_AGE_VALOR_NO_FOREIGN_HEAL'),
  ('JAC_AGE_VALOR_NO_FOREIGN_HEAL', 'Disable', '1'),
  ('JAC_AGE_VALOR_NO_FOREIGN_HEAL', 'Foreign', '1'),
  ('JAC_AGE_ATTACH_VALOR_NO_NEUTRAL_HEAL', 'ModifierId', 'JAC_AGE_VALOR_NO_NEUTRAL_HEAL'),
  ('JAC_AGE_VALOR_NO_NEUTRAL_HEAL', 'Disable', '1'),
  ('JAC_AGE_VALOR_NO_NEUTRAL_HEAL', 'Foreign', '0'),
  ('JAC_AGE_ATTACH_ELITE_EXPERIENCE', 'ModifierId', 'JAC_AGE_ELITE_EXPERIENCE'),
  ('JAC_AGE_ELITE_EXPERIENCE', 'Amount', '100'),
  ('JAC_AGE_ATTACH_ELITE_MAINTENANCE', 'ModifierId', 'JAC_AGE_ELITE_MAINTENANCE'),
  ('JAC_AGE_ELITE_MAINTENANCE', 'Amount', '-2');

-- The design's shared trait TRAIT_JAC_AGES would need a LeaderTraits row per leader of every mod; the major
-- civ trait every leader inherits (LEADER_DEFAULT, Base/Assets/Gameplay/Data/Leaders.xml) carries the official
-- age effects already (Expansion1_Leaders.xml:138-139), so the ages effects go there. Modifiers on a trait apply
-- from the game start and COLLECTION_PLAYER_CITIES includes cities founded or captured later.
INSERT INTO TraitModifiers (TraitType, ModifierId) VALUES
  ('TRAIT_LEADER_MAJOR_CIV', 'JAC_AGE_GOLDEN_PRODUCTION'),
  ('TRAIT_LEADER_MAJOR_CIV', 'JAC_AGE_GOLDEN_GOLD'),
  ('TRAIT_LEADER_MAJOR_CIV', 'JAC_AGE_TEMPLE_SCIENCE'),
  ('TRAIT_LEADER_MAJOR_CIV', 'JAC_AGE_TEMPLE_CULTURE'),
  ('TRAIT_LEADER_MAJOR_CIV', 'JAC_AGE_ATTACH_PIRATE_PRODUCTION'),
  ('TRAIT_LEADER_MAJOR_CIV', 'JAC_AGE_ATTACH_PIRATE_MOVEMENT'),
  ('TRAIT_LEADER_MAJOR_CIV', 'JAC_AGE_ATTACH_PIRATE_TRADE'),
  ('TRAIT_LEADER_MAJOR_CIV', 'JAC_AGE_ATTACH_VALOR_ATTACK'),
  ('TRAIT_LEADER_MAJOR_CIV', 'JAC_AGE_ATTACH_VALOR_NO_FOREIGN_HEAL'),
  ('TRAIT_LEADER_MAJOR_CIV', 'JAC_AGE_ATTACH_VALOR_NO_NEUTRAL_HEAL'),
  ('TRAIT_LEADER_MAJOR_CIV', 'JAC_AGE_ATTACH_ELITE_EXPERIENCE'),
  ('TRAIT_LEADER_MAJOR_CIV', 'JAC_AGE_ATTACH_ELITE_MAINTENANCE');
