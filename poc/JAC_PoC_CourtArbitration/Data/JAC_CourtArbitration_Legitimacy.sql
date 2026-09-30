-- 正当性の効果と討伐の勅命の戦争疲弊（SSOT 3.2、オーナー判断 R-1・R-2、2026-09-30）。数値は暫定。
-- 状態は Scripts/JAC_CourtArbitration.lua が都市の中心のプロットに書く 0/1 の旗（WriteCityFlags）で、ここの条件がそれを読む。
-- 条件の形は戦国パックの時代と同じ（poc/JAC_PoC_SengokuTrees/Data/JAC_Sengoku_Ages.sql）：
-- REQUIREMENT_PLOT_PROPERTY_MATCHES の公式の使用例は PropertyName と PropertyMinimum だけ
-- （DLC/CivRoyaleScenario/Data/CivRoyaleScenario_Civilizations.xml:377-403）なので、状態ごとに 0/1 の旗にする。

INSERT INTO Requirements (RequirementId, RequirementType) VALUES
  ('JAC_ARB_REQ_LEGITIMACY_HIGH', 'REQUIREMENT_PLOT_PROPERTY_MATCHES'),
  ('JAC_ARB_REQ_LEGITIMACY_LOW', 'REQUIREMENT_PLOT_PROPERTY_MATCHES'),
  ('JAC_ARB_REQ_SUBJUGATION_ANCHOR', 'REQUIREMENT_PLOT_PROPERTY_MATCHES');
INSERT INTO RequirementArguments (RequirementId, Name, Value) VALUES
  ('JAC_ARB_REQ_LEGITIMACY_HIGH', 'PropertyName', 'JAC_ARB_LEGITIMACY_HIGH'),
  ('JAC_ARB_REQ_LEGITIMACY_HIGH', 'PropertyMinimum', '1'),
  ('JAC_ARB_REQ_LEGITIMACY_LOW', 'PropertyName', 'JAC_ARB_LEGITIMACY_LOW'),
  ('JAC_ARB_REQ_LEGITIMACY_LOW', 'PropertyMinimum', '1'),
  ('JAC_ARB_REQ_SUBJUGATION_ANCHOR', 'PropertyName', 'JAC_ARB_SUBJUGATION_ANCHOR'),
  ('JAC_ARB_REQ_SUBJUGATION_ANCHOR', 'PropertyMinimum', '1');
INSERT INTO RequirementSets (RequirementSetId, RequirementSetType) VALUES
  ('JAC_ARB_CITY_LEGITIMACY_HIGH', 'REQUIREMENTSET_TEST_ALL'),
  ('JAC_ARB_CITY_LEGITIMACY_LOW', 'REQUIREMENTSET_TEST_ALL'),
  ('JAC_ARB_CITY_SUBJUGATION_ANCHOR', 'REQUIREMENTSET_TEST_ALL');
INSERT INTO RequirementSetRequirements (RequirementSetId, RequirementId) VALUES
  ('JAC_ARB_CITY_LEGITIMACY_HIGH', 'JAC_ARB_REQ_LEGITIMACY_HIGH'),
  ('JAC_ARB_CITY_LEGITIMACY_LOW', 'JAC_ARB_REQ_LEGITIMACY_LOW'),
  ('JAC_ARB_CITY_SUBJUGATION_ANCHOR', 'JAC_ARB_REQ_SUBJUGATION_ANCHOR');

-- 1. 正当性 70 以上で全都市の忠誠度 +2/ターン、30 未満で -2/ターン。全都市への忠誠度の修正値を都市の条件で絞る形は
--    公式の MODIFIER_PLAYER_CITIES_ADJUST_IDENTITY_PER_TURN + SubjectRequirementSetId
--    （DLC/Expansion1/Data/Expansion1_Civilizations_Major.xml:484-485）。
-- 2. 討伐の勅命の参加者の戦争疲弊 -50%（暫定。公式の戒厳令・宣伝は -25%：Base/Assets/Gameplay/Data/Policies.xml:836-837,
--    2919-2927 の MARTIALLAW_OVERALLWARWEARINESS、Amount と Overall）。MODIFIER_PLAYER_ADJUST_WAR_WEARINESS
--    （Base/Assets/Gameplay/Data/Modifiers.xml:84,1205-1207）は家に1つ効く種類なので、戦国パックの時代の家ごとの効果と同じく
--    MODIFIER_PLAYER_CITIES_ATTACH_MODIFIER（Modifiers.xml:780）で全都市に付け、付けた修正値の OwnerRequirementSetId で
--    代表の都市（1家に1都市）の旗が 1 の間だけ効かせる。期間が終われば旗が 0 になり効かなくなる。実機での効き方は未確認。
INSERT INTO Modifiers (ModifierId, ModifierType, OwnerRequirementSetId, SubjectRequirementSetId) VALUES
  ('JAC_ARB_LOYALTY_HIGH', 'MODIFIER_PLAYER_CITIES_ADJUST_IDENTITY_PER_TURN', NULL, 'JAC_ARB_CITY_LEGITIMACY_HIGH'),
  ('JAC_ARB_LOYALTY_LOW', 'MODIFIER_PLAYER_CITIES_ADJUST_IDENTITY_PER_TURN', NULL, 'JAC_ARB_CITY_LEGITIMACY_LOW'),
  ('JAC_ARB_ATTACH_WAR_WEARINESS', 'MODIFIER_PLAYER_CITIES_ATTACH_MODIFIER', NULL, NULL),
  ('JAC_ARB_WAR_WEARINESS', 'MODIFIER_PLAYER_ADJUST_WAR_WEARINESS', 'JAC_ARB_CITY_SUBJUGATION_ANCHOR', NULL);
INSERT INTO ModifierArguments (ModifierId, Name, Value) VALUES
  ('JAC_ARB_LOYALTY_HIGH', 'Amount', '2'),
  ('JAC_ARB_LOYALTY_LOW', 'Amount', '-2'),
  ('JAC_ARB_ATTACH_WAR_WEARINESS', 'ModifierId', 'JAC_ARB_WAR_WEARINESS'),
  ('JAC_ARB_WAR_WEARINESS', 'Amount', '-50'),
  ('JAC_ARB_WAR_WEARINESS', 'Overall', 'true');

-- 全勢力の指導者が継承する共通の特性（JAC_Sengoku_Ages.sql と同じ。後から建てた・攻め取った都市にも効く）
INSERT INTO TraitModifiers (TraitType, ModifierId) VALUES
  ('TRAIT_LEADER_MAJOR_CIV', 'JAC_ARB_LOYALTY_HIGH'),
  ('TRAIT_LEADER_MAJOR_CIV', 'JAC_ARB_LOYALTY_LOW'),
  ('TRAIT_LEADER_MAJOR_CIV', 'JAC_ARB_ATTACH_WAR_WEARINESS');
