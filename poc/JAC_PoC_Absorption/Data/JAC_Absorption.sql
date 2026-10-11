-- Outcome routing only. No probabilities, old nine states or automatic diplomatic absorption.
CREATE TABLE JAC_AbsorptionOutcomes (
  Kind TEXT NOT NULL,
  Outcome TEXT NOT NULL,
  Destination TEXT NOT NULL CHECK (Destination IN ('ABSORBER', 'ARCHIVE', 'RETURN')),
  PRIMARY KEY (Kind, Outcome)
);
INSERT INTO JAC_AbsorptionOutcomes (Kind, Outcome, Destination) VALUES
('UNIT','SURRENDER','ABSORBER'), ('UNIT','INCORPORATE','ABSORBER'),
('UNIT','DISSOLVE','ARCHIVE'), ('UNIT','ESCAPE','ARCHIVE'), ('UNIT','RESIST','ARCHIVE'),
('PERSON','CAPTURE','ABSORBER'), ('PERSON','RECRUIT','ABSORBER'), ('PERSON','SUBMIT','ABSORBER'),
('PERSON','EXILE','ARCHIVE'), ('PERSON','EXECUTE','ARCHIVE'), ('PERSON','RETIRE','ARCHIVE'), ('PERSON','DEFECT','ARCHIVE'), ('PERSON','HIDE','ARCHIVE'),
('GOVERNOR','CAPTURE','ABSORBER'), ('GOVERNOR','RECRUIT','ABSORBER'), ('GOVERNOR','SUBMIT','ABSORBER'),
('GOVERNOR','EXILE','ARCHIVE'), ('GOVERNOR','RETIRE','ARCHIVE'), ('GOVERNOR','DEFECT','ARCHIVE'), ('GOVERNOR','HIDE','ARCHIVE'),
('HERO','RECRUIT','ABSORBER'), ('HERO','PROTECT','ABSORBER'), ('HERO','RECONTRACT','ABSORBER'),
('HERO','DEFECT','ARCHIVE'), ('HERO','HIDE','ARCHIVE'), ('HERO','SUICIDE','ARCHIVE'),
('GREAT_PERSON','RECRUIT','ABSORBER'),
('GREAT_PERSON','EXILE','ARCHIVE'), ('GREAT_PERSON','RETIRE','ARCHIVE'), ('GREAT_PERSON','HIDE','ARCHIVE'),
('ITEM','TRANSFER','ABSORBER'), ('ITEM','LOOT','ABSORBER'), ('ITEM','SEIZE','ABSORBER'),
('ITEM','RETURN','RETURN'), ('ITEM','SCATTER','ARCHIVE'),
('UNLOCK','INHERIT','ABSORBER'), ('UNLOCK','PRESERVE','ARCHIVE'),
('RELIGION','PRESERVE','ABSORBER');

-- Absorption PoC needs a purchasable native cleric as a diagnostic stand-in.
-- Sengoku_Unlocks disables units outside its provisional allowlist (load order 200).
-- This action runs only with the absorption PoC in the Banseiki ruleset; it does
-- not define the final religious profession/unlock design. Keep the Shrine prerequisite.
UPDATE Units SET CanTrain = 1 WHERE UnitType = 'UNIT_MISSIONARY';

-- An internal, universal native-work slot for lossless world custody.
-- No production option or yield is attached to this building.
INSERT INTO Types (Type, Kind) VALUES ('BUILDING_JAC_ABSORPTION_CACHE', 'KIND_BUILDING');
INSERT INTO Buildings (BuildingType, Name, Description, Cost, PrereqDistrict, InternalOnly)
VALUES ('BUILDING_JAC_ABSORPTION_CACHE', 'LOC_JAC_ABS_FLOW_CACHE', 'LOC_JAC_ABS_FLOW_CACHE', 1, 'DISTRICT_CITY_CENTER', 1);
INSERT INTO GreatWorkSlotTypes (GreatWorkSlotType) VALUES ('GREATWORKSLOT_JAC_ABSORPTION_CACHE');
INSERT INTO GreatWork_ValidSubTypes (GreatWorkSlotType, GreatWorkObjectType)
SELECT 'GREATWORKSLOT_JAC_ABSORPTION_CACHE', GreatWorkObjectType FROM GreatWorkObjectTypes;
INSERT INTO Building_GreatWorks (BuildingType, GreatWorkSlotType, NumSlots)
VALUES ('BUILDING_JAC_ABSORPTION_CACHE', 'GREATWORKSLOT_JAC_ABSORPTION_CACHE', 1);

-- Native one-shot loyalty effects. The official Bread and Circuses completion
-- uses this effect with RunOnce/Permanent; callers must retain issuance receipts.
INSERT INTO Modifiers (ModifierId, ModifierType, RunOnce, Permanent) VALUES
('JAC_ABSORPTION_LOYALTY_LOSS_1', 'MODIFIER_SINGLE_CITY_GRANT_LOYALTY', 1, 1),
('JAC_ABSORPTION_LOYALTY_LOSS_2', 'MODIFIER_SINGLE_CITY_GRANT_LOYALTY', 1, 1),
('JAC_ABSORPTION_LOYALTY_LOSS_5', 'MODIFIER_SINGLE_CITY_GRANT_LOYALTY', 1, 1),
('JAC_ABSORPTION_LOYALTY_LOSS_15', 'MODIFIER_SINGLE_CITY_GRANT_LOYALTY', 1, 1);
INSERT INTO ModifierArguments (ModifierId, Name, Value) VALUES
('JAC_ABSORPTION_LOYALTY_LOSS_1', 'Amount', -1),
('JAC_ABSORPTION_LOYALTY_LOSS_2', 'Amount', -2),
('JAC_ABSORPTION_LOYALTY_LOSS_5', 'Amount', -5),
('JAC_ABSORPTION_LOYALTY_LOSS_15', 'Amount', -15);
