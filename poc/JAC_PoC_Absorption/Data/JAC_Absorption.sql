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
