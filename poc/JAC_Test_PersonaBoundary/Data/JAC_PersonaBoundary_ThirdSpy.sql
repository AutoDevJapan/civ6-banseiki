-- Test-only continuation after two saved native mission failures. The stock
-- CIVIC_GRANT_SPY modifier adds capacity through the native engine when this
-- still-unresearched Sengoku civic is completed. Product civic grants stay intact.
INSERT INTO CivicModifiers (CivicType, ModifierId)
SELECT 'CIVIC_JAC_SENGOKU_NANBAN_ENVOYS', 'CIVIC_GRANT_SPY'
WHERE EXISTS (
  SELECT 1 FROM Civics WHERE CivicType='CIVIC_JAC_SENGOKU_NANBAN_ENVOYS'
) AND NOT EXISTS (
  SELECT 1 FROM CivicModifiers
  WHERE CivicType='CIVIC_JAC_SENGOKU_NANBAN_ENVOYS'
    AND ModifierId='CIVIC_GRANT_SPY'
);
