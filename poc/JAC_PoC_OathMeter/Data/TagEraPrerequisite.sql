-- Separate opt-in: use an already granted Ancient prerequisite only in the promotion-era diagnosis.
UPDATE Units SET PrereqTech='TECH_JAC_SENGOKU_IRON_FORGING', PrereqCivic=NULL WHERE UnitType='UNIT_SPEARMAN';
