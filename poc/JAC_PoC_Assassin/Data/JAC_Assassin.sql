-- Timed city recruitment is gameplay-managed. Native hammers, purchases and upgrades cannot bypass 20 turns.
-- Base unit schema: Base/Assets/Gameplay/Data/Schema/01_GameplaySchema.sql:2833-2902.
-- OWNER_UNDECIDED PoC constants: Moves=3, Sight=2, Combat=10 (Scout stand-ins), upkeep=0.
-- Cost=1 is an unused native schema value, NOT the recruitment time/cost; CanTrain=0 for both types.
INSERT INTO Types (Type,Kind) VALUES
  ('UNIT_JAC_JAPAN_SENGOKU_ASSASSIN','KIND_UNIT'),
  ('UNIT_JAC_ASSASSIN','KIND_UNIT'),
  ('ABILITY_JAC_ASSASSIN_INFILTRATION','KIND_ABILITY');
INSERT INTO Units (UnitType,Name,Description,Cost,BaseMoves,BaseSightRange,ZoneOfControl,
  Domain,Combat,FormationClass,PromotionClass,CanTrain,CanCapture,Maintenance) VALUES
  ('UNIT_JAC_JAPAN_SENGOKU_ASSASSIN','LOC_JAC_ASN_NINJA_NAME','LOC_JAC_ASN_UNIT_DESCRIPTION',
   1,3,2,0,'DOMAIN_LAND',10,'FORMATION_CLASS_LAND_COMBAT','PROMOTION_CLASS_RECON',0,0,0),
  ('UNIT_JAC_ASSASSIN','LOC_JAC_ASN_COMMON_NAME','LOC_JAC_ASN_UNIT_DESCRIPTION',
   1,3,2,0,'DOMAIN_LAND',10,'FORMATION_CLASS_LAND_COMBAT','PROMOTION_CLASS_RECON',0,0,0);
-- A dedicated tag borrows the official foreign-lands effect without granting religion/build charges.
-- UnitAbilities.xml:552-553,948-949. Movement itself uses the normal map pathfinding.
INSERT INTO Tags (Tag,Vocabulary) VALUES ('CLASS_JAC_ASSASSIN','ABILITY_CLASS');
INSERT INTO TypeTags (Type,Tag) VALUES
  ('UNIT_JAC_JAPAN_SENGOKU_ASSASSIN','CLASS_JAC_ASSASSIN'),
  ('UNIT_JAC_ASSASSIN','CLASS_JAC_ASSASSIN'),
  ('ABILITY_JAC_ASSASSIN_INFILTRATION','CLASS_JAC_ASSASSIN');
INSERT INTO UnitAbilities (UnitAbilityType,Name,Description) VALUES
  ('ABILITY_JAC_ASSASSIN_INFILTRATION','LOC_JAC_ASN_INFILTRATION','LOC_JAC_ASN_INFILTRATION_DESCRIPTION');
INSERT INTO UnitAbilityModifiers (UnitAbilityType,ModifierId) VALUES
  ('ABILITY_JAC_ASSASSIN_INFILTRATION','MOD_ENTER_FOREIGN_LANDS');
