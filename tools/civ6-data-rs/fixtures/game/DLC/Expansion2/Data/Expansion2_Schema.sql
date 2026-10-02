-- Version 1
CREATE TABLE "AgendaTags" (
        "AgendaTagType" TEXT NOT NULL,
        PRIMARY KEY(AgendaTagType));

CREATE TABLE "Alliances" (
        "AllianceType" TEXT,
        "Name" TEXT NOT NULL DEFAULT """",
        "Description" TEXT NOT NULL DEFAULT """",
        PRIMARY KEY(AllianceType),
        FOREIGN KEY (AllianceType) REFERENCES Types(Type) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "AllianceEffects" (
        "LevelRequirement" INTEGER NOT NULL DEFAULT 0,
        "AllianceType" TEXT,
        "ModifierID" TEXT,
        PRIMARY KEY(LevelRequirement, AllianceType, ModifierID),
        FOREIGN KEY (ModifierID) REFERENCES Modifiers(ModifierId) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (AllianceType) REFERENCES Alliances(AllianceType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "Building_BuildChargeProductions" (
        "BuildingType" TEXT NOT NULL,
        "UnitType" TEXT NOT NULL,
        "PercentProductionPerCharge" INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY(BuildingType, UnitType),
        FOREIGN KEY (BuildingType) REFERENCES Buildings(BuildingType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (UnitType) REFERENCES Units(UnitType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "Building_ResourceCosts" (
        "BuildingType" TEXT NOT NULL,
        "ResourceType" TEXT NOT NULL,
        "StartProductionCost" INTEGER NOT NULL,
        "PerTurnMaintenanceCost" INTEGER NOT NULL,
        PRIMARY KEY(BuildingType, ResourceType),
        FOREIGN KEY (BuildingType) REFERENCES Buildings(BuildingType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (ResourceType) REFERENCES Resources(ResourceType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "Building_TourismBombs_XP2" (
        "BuildingType" TEXT NOT NULL,
        "TourismBombValue" INTEGER NOT NULL,
        PRIMARY KEY(BuildingType),
        FOREIGN KEY (BuildingType) REFERENCES Buildings(BuildingType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "Buildings_XP2" (
        "BuildingType" TEXT NOT NULL,
        "RequiredPower" INTEGER NOT NULL DEFAULT 0,
        "ResourceTypeConvertedToPower" TEXT,
        "PreventsFloods" BOOLEAN NOT NULL CHECK (PreventsFloods IN (0,1)) DEFAULT 0,
        "PreventsDrought" BOOLEAN NOT NULL CHECK (PreventsDrought IN (0,1)) DEFAULT 0,
        "BlocksCoastalFlooding" BOOLEAN NOT NULL CHECK (BlocksCoastalFlooding IN (0,1)) DEFAULT 0,
        "CostMultiplierPerTile" INTEGER NOT NULL DEFAULT 0,
        "CostMultiplierPerSeaLevel" INTEGER NOT NULL DEFAULT 0,
        "Bridge" BOOLEAN NOT NULL CHECK (Bridge IN (0,1)) DEFAULT 0,
        "CanalWonder" BOOLEAN NOT NULL CHECK (CanalWonder IN (0,1)) DEFAULT 0,
        "EntertainmentBonusWithPower" INTEGER NOT NULL DEFAULT 0,
        "NuclearReactor" BOOLEAN NOT NULL CHECK (NuclearReactor IN (0,1)) DEFAULT 0,
        "Pillage" BOOLEAN NOT NULL CHECK (Pillage IN (0,1)) DEFAULT 1,
        PRIMARY KEY(BuildingType),
        FOREIGN KEY (BuildingType) REFERENCES Buildings(BuildingType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (ResourceTypeConvertedToPower) REFERENCES Resources(ResourceType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "Building_YieldChangesBonusWithPower" (
        "BuildingType" TEXT NOT NULL,
        "YieldType" TEXT NOT NULL,
        "YieldChange" INTEGER NOT NULL,
        PRIMARY KEY(BuildingType, YieldType),
        FOREIGN KEY (BuildingType) REFERENCES Buildings(BuildingType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (YieldType) REFERENCES Yields(YieldType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "CoastalLowlands" (
        "CoastalLowlandType" TEXT NOT NULL,
        "Name" TEXT NOT NULL,
        "Description" TEXT NOT NULL,
        "FloodedEvent" TEXT NOT NULL,
        "SubmergedEvent" TEXT NOT NULL,
        PRIMARY KEY(CoastalLowlandType),
        FOREIGN KEY (FloodedEvent) REFERENCES RandomEvents(RandomEventType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (SubmergedEvent) REFERENCES RandomEvents(RandomEventType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "CommemorationModifiers" (
        "CommemorationType" TEXT NOT NULL,
        "ModifierId" TEXT NOT NULL,
        PRIMARY KEY(CommemorationType, ModifierId),
        FOREIGN KEY (CommemorationType) REFERENCES CommemorationTypes(CommemorationType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "CommemorationTypes" (
        "CommemorationType" TEXT NOT NULL,
        "CategoryDescription" LocalizedText,
        "GoldenAgeBonusDescription" LocalizedText,
        "NormalAgeBonusDescription" LocalizedText,
        "DarkAgeBonusDescription" LocalizedText,
        "MinimumGameEra" TEXT,
        "MaximumGameEra" TEXT,
        PRIMARY KEY(CommemorationType),
        FOREIGN KEY (MinimumGameEra) REFERENCES Eras(EraType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (MaximumGameEra) REFERENCES Eras(EraType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "ComplimentModifiers" (
        "CommemorationType" TEXT NOT NULL,
        "ModifierId" TEXT NOT NULL,
        PRIMARY KEY(CommemorationType, ModifierId),
        FOREIGN KEY (CommemorationType) REFERENCES CommemorationTypes(CommemorationType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "CongressAiChanges" (
        "ResolutionType" TEXT,
        "DiscussionType" TEXT,
        "YieldType" TEXT,
        "PseudoYieldType" TEXT,
        "Value" INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY(ResolutionType, DiscussionType, YieldType, PseudoYieldType),
        FOREIGN KEY (ResolutionType) REFERENCES Resolutions(ResolutionType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (DiscussionType) REFERENCES Discussions(DiscussionType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (YieldType) REFERENCES Yields(YieldType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (PseudoYieldType) REFERENCES PseudoYields(PseudoYieldType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "DeforestationEffects" (
        "DeforestationEffectType" TEXT NOT NULL,
        "Name" TEXT,
        "MaxAverageDeforestation" REAL NOT NULL DEFAULT 0,
        "CO2PercentModifier" INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY(DeforestationEffectType));

CREATE TABLE "DeforestationLevels" (
        "DeforestationType" TEXT NOT NULL UNIQUE,
        "Name" TEXT NOT NULL,
        "Description" TEXT NOT NULL,
        "MaxDeforestationPercent" INTEGER NOT NULL DEFAULT 0,
        "DeforestationPointsPerTurn" INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY(DeforestationType));

CREATE TABLE "DiplomaticActions_XP1" (
        "DiplomaticActionType" TEXT NOT NULL,
        "RequiresGoldenAgeCommemorationType" TEXT,
        "AllianceType" TEXT,
        "RequiresBrokenPromise" BOOLEAN NOT NULL CHECK (RequiresBrokenPromise IN (0,1)) DEFAULT 0,
        "RequiresDifferentLateGovernment" BOOLEAN NOT NULL CHECK (RequiresDifferentLateGovernment IN (0,1)) DEFAULT 0,
        "RequiresAllianceSoonToEnd" BOOLEAN NOT NULL CHECK (RequiresAllianceSoonToEnd IN (0,1)) DEFAULT 0,
        PRIMARY KEY(DiplomaticActionType),
        FOREIGN KEY (DiplomaticActionType) REFERENCES DiplomaticActions(DiplomaticActionType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (RequiresGoldenAgeCommemorationType) REFERENCES CommemorationTypes(CommemorationType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (AllianceType) REFERENCES Alliances(AllianceType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "DiplomaticActions_XP2" (
        "DiplomaticActionType" TEXT NOT NULL,
        "FavorCost" INTEGER NOT NULL DEFAULT 0,
        "GrievanceCost" INTEGER NOT NULL DEFAULT 0,
        "PromiseType" TEXT,
        "RequiredPromise" TEXT,
        "GrievancesForRefusal" INTEGER NOT NULL DEFAULT 0,
        "GrievancesPerIncursion" INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY(DiplomaticActionType),
        FOREIGN KEY (DiplomaticActionType) REFERENCES DiplomaticActions(DiplomaticActionType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "DiplomaticVisibilitySources_XP1" (
        "VisibilitySourceType" TEXT NOT NULL,
        "TradePostTrait" TEXT,
        PRIMARY KEY(VisibilitySourceType),
        FOREIGN KEY (VisibilitySourceType) REFERENCES DiplomaticVisibilitySources(VisibilitySourceType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (TradePostTrait) REFERENCES Traits(TraitType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "Discussions" (
        "DiscussionType" TEXT NOT NULL,
        "ProposalType" TEXT NOT NULL,
        "Name" TEXT NOT NULL,
        "Description" TEXT NOT NULL,
        "EmergencyType" TEXT,
        PRIMARY KEY(DiscussionType),
        FOREIGN KEY (DiscussionType) REFERENCES Types(Type) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (ProposalType) REFERENCES ProposalTypes(ProposalType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (EmergencyType) REFERENCES Emergencies_XP2(EmergencyType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "District_BuildChargeProductions" (
        "DistrictType" TEXT NOT NULL,
        "UnitType" TEXT NOT NULL,
        "PercentProductionPerCharge" INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY(DistrictType, UnitType),
        FOREIGN KEY (DistrictType) REFERENCES Districts(DistrictType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (UnitType) REFERENCES Units(UnitType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "Districts_XP2" (
        "DistrictType" TEXT NOT NULL,
        "OnePerRiver" BOOLEAN NOT NULL CHECK (OnePerRiver IN (0,1)) DEFAULT 0,
        "PreventsFloods" BOOLEAN NOT NULL CHECK (PreventsFloods IN (0,1)) DEFAULT 0,
        "PreventsDrought" BOOLEAN NOT NULL CHECK (PreventsDrought IN (0,1)) DEFAULT 0,
        "Canal" BOOLEAN NOT NULL CHECK (Canal IN (0,1)) DEFAULT 0,
        "AttackRange" INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY(DistrictType),
        FOREIGN KEY (DistrictType) REFERENCES Districts(DistrictType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "EmergencyAlliances" (
        "EmergencyType" TEXT NOT NULL,
        "Trigger" TEXT NOT NULL,
        "TargetRequirementSet" TEXT,
        "GoalTrigger" TEXT NOT NULL,
        "MemberRequirementSet" TEXT,
        "Duration" INTEGER NOT NULL DEFAULT 0,
        "RemovesWarState" BOOLEAN NOT NULL CHECK (RemovesWarState IN (0,1)) DEFAULT 1,
        "ShareVis" BOOLEAN NOT NULL CHECK (ShareVis IN (0,1)) DEFAULT 0,
        "OpenBorders" BOOLEAN NOT NULL CHECK (OpenBorders IN (0,1)) DEFAULT 0,
        "KillFriendship" BOOLEAN NOT NULL CHECK (KillFriendship IN (0,1)) DEFAULT 1,
        "WarOnTarget" BOOLEAN NOT NULL CHECK (WarOnTarget IN (0,1)) DEFAULT 1,
        "InformTarget" BOOLEAN NOT NULL CHECK (InformTarget IN (0,1)) DEFAULT 1,
        "LockoutTime" INTEGER NOT NULL DEFAULT 0,
        "EmergencyText" TEXT NOT NULL,
        "GoalText" TEXT NOT NULL,
        "Name" TEXT NOT NULL,
        PRIMARY KEY(EmergencyType),
        FOREIGN KEY (MemberRequirementSet) REFERENCES RequirementSets(RequirementSetId) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (EmergencyType) REFERENCES Types(Type) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (EmergencyText) REFERENCES EmergencyTexts(Type) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (GoalText) REFERENCES EmergencyGoalTexts(GoalType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "Emergencies_XP2" (
        "EmergencyType" TEXT,
        "Hostile" BOOLEAN NOT NULL CHECK (Hostile IN (0,1)) DEFAULT 1,
        "NoTarget" BOOLEAN NOT NULL CHECK (NoTarget IN (0,1)) DEFAULT 0,
        "UIType" TEXT,
        "UIBackgroundPrefix" TEXT,
        "NoCongress" BOOLEAN NOT NULL CHECK (NoCongress IN (0,1)) DEFAULT 0,
        PRIMARY KEY(EmergencyType),
        FOREIGN KEY (EmergencyType) REFERENCES EmergencyAlliances(EmergencyType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "EmergencyBuffs" (
        "ModifierID" TEXT NOT NULL,
        "EmergencyType" TEXT NOT NULL,
        "Description" TEXT,
        PRIMARY KEY(ModifierID, EmergencyType),
        FOREIGN KEY (EmergencyType) REFERENCES EmergencyAlliances(EmergencyType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (ModifierID) REFERENCES Modifiers(ModifierId) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "EmergencyGoalTexts" (
        "GoalType" TEXT NOT NULL UNIQUE,
        "GoalDescription" TEXT,
        "ShortGoalDescription" TEXT,
        "TentativeGoalDescription" TEXT,
        "ShortTargetGoalDescription" TEXT,
        "ListGoal" TEXT,
        "TargetListGoal" TEXT,
        PRIMARY KEY(GoalType));

CREATE TABLE "EmergencyRewards" (
        "ModifierID" TEXT NOT NULL,
        "EmergencyType" TEXT NOT NULL,
        "OnSuccess" BOOLEAN NOT NULL CHECK (OnSuccess IN (0,1)),
        "Description" TEXT,
        "FirstPlace" BOOLEAN NOT NULL CHECK (FirstPlace IN (0,1)) DEFAULT 0,
        "TopTier" BOOLEAN NOT NULL CHECK (TopTier IN (0,1)) DEFAULT 0,
        "BottomTier" BOOLEAN NOT NULL CHECK (BottomTier IN (0,1)) DEFAULT 0,
        PRIMARY KEY(ModifierID, EmergencyType, OnSuccess),
        FOREIGN KEY (EmergencyType) REFERENCES EmergencyAlliances(EmergencyType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (ModifierID) REFERENCES Modifiers(ModifierId) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "EmergencyScoreSources" (
        "ScoreSourceType" TEXT,
        "EmergencyType" TEXT NOT NULL,
        "Description" TEXT,
        "ScoreAmount" INTEGER NOT NULL DEFAULT 0,
        "FromProject" TEXT,
        "MilitaryInEnemyTerritory" BOOLEAN NOT NULL CHECK (MilitaryInEnemyTerritory IN (0,1)) DEFAULT 0,
        "ReligiousInEnemyTerritory" BOOLEAN NOT NULL CHECK (ReligiousInEnemyTerritory IN (0,1)) DEFAULT 0,
        "AttackedEnemyCity" BOOLEAN NOT NULL CHECK (AttackedEnemyCity IN (0,1)) DEFAULT 0,
        "FromGold" BOOLEAN NOT NULL CHECK (FromGold IN (0,1)) DEFAULT 0,
        "KilledEnemyUnit" BOOLEAN NOT NULL CHECK (KilledEnemyUnit IN (0,1)) DEFAULT 0,
        "SpreadReligion" BOOLEAN NOT NULL CHECK (SpreadReligion IN (0,1)) DEFAULT 0,
        "ReligionAttackedEnemy" BOOLEAN NOT NULL CHECK (ReligionAttackedEnemy IN (0,1)) DEFAULT 0,
        "ReligiousUnitsInEnemyTerritory" BOOLEAN NOT NULL CHECK (ReligiousUnitsInEnemyTerritory IN (0,1)) DEFAULT 0,
        "FromGreatPerson" TEXT,
        "FromFavor" BOOLEAN NOT NULL CHECK (FromFavor IN (0,1)) DEFAULT 0,
        "FromBuilding" TEXT,
        "FromDistrict" TEXT,
        "FromCO2Footprint" BOOLEAN NOT NULL CHECK (FromCO2Footprint IN (0,1)) DEFAULT 0,
        "FromAtWar" BOOLEAN NOT NULL CHECK (FromAtWar IN (0,1)) DEFAULT 0,
        "FromBadCO2Footprint" BOOLEAN NOT NULL CHECK (FromBadCO2Footprint IN (0,1)) DEFAULT 0,
        "FromSacrificedUnitStrength" BOOLEAN NOT NULL CHECK (FromSacrificedUnitStrength IN (0,1)) DEFAULT 0,
        PRIMARY KEY(ScoreSourceType),
        FOREIGN KEY (FromProject) REFERENCES Projects(ProjectType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (EmergencyType) REFERENCES Emergencies_XP2(EmergencyType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (FromGreatPerson) REFERENCES GreatPersonClasses(GreatPersonClassType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (FromDistrict) REFERENCES Districts(DistrictType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (FromBuilding) REFERENCES Buildings(BuildingType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (FromProject) REFERENCES Projects_XP2(ProjectType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "EmergencyTexts" (
        "Type" TEXT NOT NULL UNIQUE,
        "Flavor" TEXT,
        "Description" TEXT,
        "ExtraEffects" TEXT,
        "ExtraRewards" TEXT,
        "ExtraFailureRewards" TEXT,
        "DescriptionShorter" TEXT,
        PRIMARY KEY(Type));

CREATE TABLE "Eras_XP1" (
        "EraType" TEXT NOT NULL,
        "GameEraMinimumTurns" INTEGER,
        "GameEraMaximumTurns" INTEGER,
        "LiberatedEnvoys" INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY(EraType),
        FOREIGN KEY (EraType) REFERENCES Eras(EraType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "Eras_XP2" (
        "EraType" TEXT NOT NULL,
        "GrievanceDecayRate" INTEGER NOT NULL DEFAULT 0,
        "TensionDecayRate" INTEGER NOT NULL DEFAULT 0,
        "TradeRouteMinimumEndTurnChange" INTEGER,
        "EraScoreThresholdShift" INTEGER,
        PRIMARY KEY(EraType));

CREATE TABLE "Feature_Floodplains" (
        "FeatureType" TEXT NOT NULL,
        PRIMARY KEY(FeatureType),
        FOREIGN KEY (FeatureType) REFERENCES Features(FeatureType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "Features_XP2" (
        "FeatureType" TEXT NOT NULL,
        "Volcano" BOOLEAN NOT NULL CHECK (Volcano IN (0,1)) DEFAULT 0,
        "ValidWonderPlacement" BOOLEAN NOT NULL CHECK (ValidWonderPlacement IN (0,1)) DEFAULT 0,
        "ValidDistrictPlacement" BOOLEAN NOT NULL CHECK (ValidDistrictPlacement IN (0,1)) DEFAULT 0,
        "Eruptable" BOOLEAN NOT NULL CHECK (Eruptable IN (0,1)) DEFAULT 0,
        "ValidForReplacement" BOOLEAN NOT NULL CHECK (ValidForReplacement IN (0,1)) DEFAULT 0,
        PRIMARY KEY(FeatureType),
        FOREIGN KEY (FeatureType) REFERENCES Features(FeatureType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "GoodyHutSubTypes_XP2" (
        "SubTypeGoodyHut" TEXT NOT NULL,
        "CityState" BOOLEAN NOT NULL CHECK (CityState IN (0,1)) DEFAULT 0,
        "StrategicResources" BOOLEAN NOT NULL CHECK (StrategicResources IN (0,1)) DEFAULT 0,
        PRIMARY KEY(SubTypeGoodyHut),
        FOREIGN KEY (SubTypeGoodyHut) REFERENCES GoodyHutSubTypes(SubTypeGoodyHut) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "Governments_XP2" (
        "GovernmentType" TEXT NOT NULL,
        "Favor" INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY(GovernmentType),
        FOREIGN KEY (GovernmentType) REFERENCES Governments(GovernmentType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "Governors" (
        "GovernorType" TEXT NOT NULL,
        "Name" LocalizedText NOT NULL,
        "Description" LocalizedText NOT NULL,
        "IdentityPressure" INTEGER NOT NULL DEFAULT 0,
        "Title" LocalizedText NOT NULL,
        "ShortTitle" LocalizedText NOT NULL,
        "TransitionStrength" INTEGER NOT NULL DEFAULT 0,
        "AssignCityState" BOOLEAN NOT NULL CHECK (AssignCityState IN (0,1)) DEFAULT 0,
        "Image" TEXT NOT NULL DEFAULT "NO_IMAGE",
        "PortraitImage" TEXT NOT NULL,
        "PortraitImageSelected" TEXT NOT NULL,
        "TraitType" TEXT,
        PRIMARY KEY(GovernorType),
        FOREIGN KEY (GovernorType) REFERENCES Types(Type) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (TraitType) REFERENCES Traits(TraitType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "Governors_XP2" (
        "GovernorType" TEXT NOT NULL,
        "AssignToMajor" BOOLEAN NOT NULL CHECK (AssignToMajor IN (0,1)) DEFAULT 0,
        PRIMARY KEY(GovernorType),
        FOREIGN KEY (GovernorType) REFERENCES Governors(GovernorType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "GovernorsCannotAssign" (
        "GovernorType" TEXT NOT NULL,
        "CannotAssign" BOOLEAN NOT NULL CHECK (CannotAssign IN (0,1)),
        PRIMARY KEY(GovernorType),
        FOREIGN KEY (GovernorType) REFERENCES Governors(GovernorType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "GovernorModifiers" (
        "GovernorType" TEXT NOT NULL,
        "ModifierId" TEXT NOT NULL,
        PRIMARY KEY(GovernorType, ModifierId),
        FOREIGN KEY (GovernorType) REFERENCES Governors(GovernorType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "GovernorPromotions" (
        "GovernorPromotionType" TEXT NOT NULL,
        "Name" TEXT NOT NULL,
        "Description" TEXT NOT NULL,
        "Level" INTEGER NOT NULL DEFAULT 0,
        "Column" INTEGER NOT NULL DEFAULT 0,
        "BaseAbility" BOOLEAN NOT NULL CHECK (BaseAbility IN (0,1)) DEFAULT 0,
        PRIMARY KEY(GovernorPromotionType));

CREATE TABLE "GovernorPromotionConditions" (
        "GovernorPromotionType" TEXT NOT NULL,
        "HiddenWithoutPrereqs" BOOLEAN NOT NULL CHECK (HiddenWithoutPrereqs IN (0,1)),
        "EarliestGameEra" TEXT,
        PRIMARY KEY(GovernorPromotionType),
        FOREIGN KEY (EarliestGameEra) REFERENCES Eras(EraType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (GovernorPromotionType) REFERENCES GovernorPromotions(GovernorPromotionType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "GovernorPromotionModifiers" (
        "GovernorPromotionType" TEXT NOT NULL,
        "ModifierId" TEXT NOT NULL,
        PRIMARY KEY(GovernorPromotionType, ModifierId),
        FOREIGN KEY (GovernorPromotionType) REFERENCES GovernorPromotions(GovernorPromotionType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "GovernorPromotionPrereqs" (
        "GovernorPromotionType" TEXT NOT NULL,
        "PrereqGovernorPromotion" TEXT NOT NULL,
        PRIMARY KEY(GovernorPromotionType, PrereqGovernorPromotion),
        FOREIGN KEY (GovernorPromotionType) REFERENCES GovernorPromotions(GovernorPromotionType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (PrereqGovernorPromotion) REFERENCES GovernorPromotions(GovernorPromotionType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "GovernorPromotionSets" (
        "GovernorType" TEXT NOT NULL,
        "GovernorPromotion" TEXT NOT NULL,
        PRIMARY KEY(GovernorType, GovernorPromotion),
        FOREIGN KEY (GovernorType) REFERENCES Governors(GovernorType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (GovernorPromotion) REFERENCES GovernorPromotions(GovernorPromotionType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "GovernorReplaces" (
        "UniqueGovernorType" TEXT NOT NULL,
        "ReplacesGovernorType" TEXT NOT NULL,
        PRIMARY KEY(UniqueGovernorType, ReplacesGovernorType),
        FOREIGN KEY (UniqueGovernorType) REFERENCES Governors(GovernorType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (ReplacesGovernorType) REFERENCES Governors(GovernorType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "GreatWorks_MODE" (
        "GreatWorkType" TEXT NOT NULL,
        "RequiredGovernor" TEXT,
        PRIMARY KEY(GreatWorkType),
        FOREIGN KEY (GreatWorkType) REFERENCES GreatWorks(GreatWorkType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (RequiredGovernor) REFERENCES Governors(GovernorType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "Happinesses_XP1" (
        "HappinessType" TEXT NOT NULL,
        "IdentityPerTurnChange" INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY(HappinessType),
        FOREIGN KEY (HappinessType) REFERENCES Happinesses(HappinessType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "Improvements_XP2" (
        "ImprovementType" TEXT NOT NULL,
        "AllowImpassableMovement" BOOLEAN NOT NULL CHECK (AllowImpassableMovement IN (0,1)) DEFAULT 0,
        "BuildOnAdjacentPlot" BOOLEAN NOT NULL CHECK (BuildOnAdjacentPlot IN (0,1)) DEFAULT 0,
        "PreventsDrought" BOOLEAN NOT NULL CHECK (PreventsDrought IN (0,1)) DEFAULT 0,
        "DisasterResistant" BOOLEAN NOT NULL CHECK (DisasterResistant IN (0,1)) DEFAULT 0,
        PRIMARY KEY(ImprovementType),
        FOREIGN KEY (ImprovementType) REFERENCES Improvements(ImprovementType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "Leaders_XP2" (
        "LeaderType" TEXT NOT NULL,
        "OceanStart" BOOLEAN NOT NULL CHECK (OceanStart IN (0,1)) DEFAULT 0,
        "MinorCivBonusType" TEXT,
        PRIMARY KEY(LeaderType),
        FOREIGN KEY (MinorCivBonusType) REFERENCES MinorCivBonuses(MinorCivBonusType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "LoyaltyLevels" (
        "LoyaltyLevelType" TEXT NOT NULL,
        "YieldChange" REAL NOT NULL DEFAULT 0,
        "GrowthChange" REAL NOT NULL DEFAULT 0,
        "Name" TEXT NOT NULL,
        "Description" TEXT,
        "LoyaltyMax" INTEGER NOT NULL DEFAULT 0,
        "LoyaltyMin" INTEGER NOT NULL DEFAULT 0,
        "IdentityChange" REAL NOT NULL DEFAULT 0,
        PRIMARY KEY(LoyaltyLevelType));

CREATE TABLE "Maps_XP2" (
        "MapSizeType" TEXT NOT NULL,
        "CO2For1DegreeTempRise" INTEGER NOT NULL DEFAULT 0,
        "DesertPlotCountToLabel" INTEGER NOT NULL DEFAULT 0,
        "MountainPlotCountToLabel" INTEGER NOT NULL DEFAULT 0,
        "SeaPlotCountToLabel" INTEGER NOT NULL DEFAULT 0,
        "LakePlotCountToLabel" INTEGER NOT NULL DEFAULT 0,
        "OceanPlotCountToLabel" INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY(MapSizeType),
        FOREIGN KEY (MapSizeType) REFERENCES Maps(MapSizeType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "MinorCivBonuses" (
        "MinorCivBonusType" TEXT NOT NULL,
        "Name" LocalizedText NOT NULL,
        PRIMARY KEY(MinorCivBonusType));

CREATE TABLE "Moments" (
        "MomentType" TEXT NOT NULL,
        "Name" LocalizedText NOT NULL,
        "Description" LocalizedText NOT NULL,
        "InstanceDescription" LocalizedText,
        "InterestLevel" INTEGER NOT NULL DEFAULT 0,
        "EraScore" INTEGER,
        "RepeatTurnCooldown" INTEGER,
        "CommemorationType" TEXT,
        "MinimumGameEra" TEXT,
        "MaximumGameEra" TEXT,
        "BackgroundTexture" TEXT,
        "IconTexture" TEXT,
        "MomentIllustrationType" TEXT,
        "ObsoleteEra" TEXT,
        PRIMARY KEY(MomentType),
        FOREIGN KEY (MomentType) REFERENCES Types(Type) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (CommemorationType) REFERENCES CommemorationTypes(CommemorationType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (MinimumGameEra) REFERENCES Eras(EraType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (MaximumGameEra) REFERENCES Eras(EraType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (MomentIllustrationType) REFERENCES MomentIllustrationTypes(MomentIllustrationType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (ObsoleteEra) REFERENCES Eras(EraType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "MomentDataTypes" (
        "MomentDataType" TEXT NOT NULL,
        "Name" INTEGER,
        PRIMARY KEY(MomentDataType));

CREATE TABLE "MomentIllustrations" (
        "MomentIllustrationType" TEXT NOT NULL,
        "MomentDataType" TEXT NOT NULL,
        "GameDataType" TEXT NOT NULL,
        "Texture" TEXT NOT NULL,
        PRIMARY KEY(MomentIllustrationType, MomentDataType, GameDataType),
        FOREIGN KEY (GameDataType) REFERENCES Types(Type) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (MomentIllustrationType) REFERENCES MomentIllustrationTypes(MomentIllustrationType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (MomentDataType) REFERENCES MomentDataTypes(MomentDataType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "MomentIllustrationTypes" (
        "MomentIllustrationType" TEXT NOT NULL UNIQUE,
        PRIMARY KEY(MomentIllustrationType));

CREATE TABLE "NamedDeserts" (
        "NamedDesertType" TEXT NOT NULL,
        "Name" TEXT NOT NULL,
        PRIMARY KEY(NamedDesertType));

CREATE TABLE "NamedDesertCivilizations" (
        "NamedDesertType" TEXT NOT NULL,
        "CivilizationType" TEXT NOT NULL,
        PRIMARY KEY(NamedDesertType, CivilizationType),
        FOREIGN KEY (NamedDesertType) REFERENCES NamedDeserts(NamedDesertType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (CivilizationType) REFERENCES Civilizations(CivilizationType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "NamedLakes" (
        "NamedLakeType" TEXT NOT NULL,
        "Name" TEXT NOT NULL,
        PRIMARY KEY(NamedLakeType));

CREATE TABLE "NamedLakeCivilizations" (
        "NamedLakeType" TEXT NOT NULL,
        "CivilizationType" TEXT NOT NULL,
        PRIMARY KEY(NamedLakeType, CivilizationType),
        FOREIGN KEY (NamedLakeType) REFERENCES NamedLakes(NamedLakeType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (CivilizationType) REFERENCES Civilizations(CivilizationType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "NamedMountains" (
        "NamedMountainType" TEXT NOT NULL,
        "Name" TEXT NOT NULL,
        PRIMARY KEY(NamedMountainType));

CREATE TABLE "NamedMountainCivilizations" (
        "NamedMountainType" TEXT NOT NULL,
        "CivilizationType" TEXT NOT NULL,
        PRIMARY KEY(NamedMountainType, CivilizationType),
        FOREIGN KEY (NamedMountainType) REFERENCES NamedMountains(NamedMountainType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (CivilizationType) REFERENCES Civilizations(CivilizationType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "NamedOceans" (
        "NamedOceanType" TEXT NOT NULL,
        "Name" TEXT NOT NULL,
        PRIMARY KEY(NamedOceanType));

CREATE TABLE "NamedOceanCivilizations" (
        "NamedOceanType" TEXT NOT NULL,
        "CivilizationType" TEXT NOT NULL,
        PRIMARY KEY(NamedOceanType, CivilizationType),
        FOREIGN KEY (NamedOceanType) REFERENCES NamedOceans(NamedOceanType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (CivilizationType) REFERENCES Civilizations(CivilizationType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "NamedRivers" (
        "NamedRiverType" TEXT NOT NULL,
        "Name" TEXT NOT NULL,
        PRIMARY KEY(NamedRiverType));

CREATE TABLE "NamedRiverCivilizations" (
        "NamedRiverType" TEXT NOT NULL,
        "CivilizationType" TEXT NOT NULL,
        PRIMARY KEY(NamedRiverType, CivilizationType),
        FOREIGN KEY (NamedRiverType) REFERENCES NamedRivers(NamedRiverType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (CivilizationType) REFERENCES Civilizations(CivilizationType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "NamedSeas" (
        "NamedSeaType" TEXT NOT NULL,
        "Name" TEXT NOT NULL,
        PRIMARY KEY(NamedSeaType));

CREATE TABLE "NamedSeaCivilizations" (
        "NamedSeaType" TEXT NOT NULL,
        "CivilizationType" TEXT NOT NULL,
        PRIMARY KEY(NamedSeaType, CivilizationType),
        FOREIGN KEY (NamedSeaType) REFERENCES NamedSeas(NamedSeaType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (CivilizationType) REFERENCES Civilizations(CivilizationType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "NamedVolcanoes" (
        "NamedVolcanoType" TEXT NOT NULL,
        "Name" TEXT NOT NULL,
        PRIMARY KEY(NamedVolcanoType));

CREATE TABLE "NamedVolcanoCivilizations" (
        "NamedVolcanoType" TEXT NOT NULL,
        "CivilizationType" TEXT NOT NULL,
        PRIMARY KEY(NamedVolcanoType, CivilizationType),
        FOREIGN KEY (NamedVolcanoType) REFERENCES NamedVolcanoes(NamedVolcanoType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (CivilizationType) REFERENCES Civilizations(CivilizationType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "Policy_GovernmentExclusives_XP2" (
        "PolicyType" TEXT,
        "GovernmentType" TEXT,
        PRIMARY KEY(PolicyType, GovernmentType),
        FOREIGN KEY (GovernmentType) REFERENCES Governments(GovernmentType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (PolicyType) REFERENCES Policies(PolicyType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "Policies_XP1" (
        "PolicyType" TEXT NOT NULL,
        "MinimumGameEra" TEXT,
        "MaximumGameEra" TEXT,
        "RequiresDarkAge" BOOLEAN NOT NULL CHECK (RequiresDarkAge IN (0,1)) DEFAULT 0,
        "RequiresGoldenAge" BOOLEAN NOT NULL CHECK (RequiresGoldenAge IN (0,1)) DEFAULT 0,
        PRIMARY KEY(PolicyType),
        FOREIGN KEY (MinimumGameEra) REFERENCES Eras(EraType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (MaximumGameEra) REFERENCES Eras(EraType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "PrevailingWinds" (
        "MinimumLatitude" INTEGER NOT NULL DEFAULT 0,
        "MaximumLatitude" INTEGER NOT NULL DEFAULT 0,
        "DirectionType" TEXT NOT NULL DEFAULT "DIRECTION_WEST",
        "Weight" INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY(MinimumLatitude, MaximumLatitude, DirectionType));

-- Buildings that must be spent/destroyed when the project is completed. Only one is required to start the project, but all are removed (intended use for mutually exclusive buildings).
CREATE TABLE "Project_BuildingCosts" (
        "ProjectType" TEXT NOT NULL,
        "ConsumedBuildingType" TEXT NOT NULL,
        PRIMARY KEY(ProjectType, ConsumedBuildingType),
        FOREIGN KEY (ProjectType) REFERENCES Projects(ProjectType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (ConsumedBuildingType) REFERENCES Buildings(BuildingType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (ProjectType) REFERENCES Projects_XP2(ProjectType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "Project_ResourceCosts" (
        "ProjectType" TEXT NOT NULL,
        "ResourceType" TEXT NOT NULL,
        "StartProductionCost" INTEGER NOT NULL,
        PRIMARY KEY(ProjectType, ResourceType),
        FOREIGN KEY (ProjectType) REFERENCES Projects(ProjectType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (ResourceType) REFERENCES Resources(ResourceType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "Projects_XP1" (
        "ProjectType" TEXT NOT NULL,
        "IdentityPerCitizenChange" REAL NOT NULL DEFAULT 0,
        "UnlocksFromEffect" BOOLEAN NOT NULL CHECK (UnlocksFromEffect IN (0,1)) DEFAULT 0,
        PRIMARY KEY(ProjectType),
        FOREIGN KEY (ProjectType) REFERENCES Projects(ProjectType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "Projects_XP2" (
        "ProjectType" TEXT NOT NULL,
        "RequiredPowerWhileActive" INTEGER NOT NULL DEFAULT 0,
        "ReligiousPressureModifier" INTEGER NOT NULL DEFAULT 0,
        "UnlocksFromEffect" BOOLEAN NOT NULL CHECK (UnlocksFromEffect IN (0,1)) DEFAULT 0,
        "RequiredBuilding" TEXT,
        "CreateBuilding" TEXT,
        "FullyPoweredWhileActive" BOOLEAN CHECK (FullyPoweredWhileActive IN (0,1)),
        "MaxSimultaneousInstances" INTEGER,
        PRIMARY KEY(ProjectType),
        FOREIGN KEY (ProjectType) REFERENCES Projects(ProjectType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (RequiredBuilding) REFERENCES Buildings(BuildingType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (CreateBuilding) REFERENCES Buildings(BuildingType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "ProposalBlockers" (
        "ProposalBlockerType" TEXT NOT NULL,
        "Description" TEXT NOT NULL,
        PRIMARY KEY(ProposalBlockerType),
        FOREIGN KEY (ProposalBlockerType) REFERENCES Types(Type) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "ProposalTypes" (
        "ProposalType" TEXT,
        "Icon" INTEGER,
        "Name" TEXT NOT NULL,
        "Description" TEXT NOT NULL,
        "BigVersion" BOOLEAN NOT NULL CHECK (BigVersion IN (0,1)) DEFAULT 0,
        "Sort" INTEGER NOT NULL,
        PRIMARY KEY(ProposalType),
        FOREIGN KEY (ProposalType) REFERENCES Types(Type) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "RandomAgendas_XP2" (
        "AgendaType" TEXT NOT NULL,
        "AgendaTag" TEXT NOT NULL,
        "RequiresReligion" BOOLEAN NOT NULL CHECK (RequiresReligion IN (0,1)) DEFAULT 0,
        PRIMARY KEY(AgendaType),
        FOREIGN KEY (AgendaType) REFERENCES Agendas(AgendaType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (AgendaTag) REFERENCES AgendaTags(AgendaTagType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "RandomAgendaCivicTags" (
        "CivicType" TEXT NOT NULL,
        "AgendaTag" TEXT NOT NULL,
        PRIMARY KEY(CivicType, AgendaTag),
        FOREIGN KEY (CivicType) REFERENCES Civics(CivicType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (AgendaTag) REFERENCES AgendaTags(AgendaTagType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "RandomAgendaEraTags" (
        "EraType" TEXT NOT NULL,
        "AgendaTag" TEXT NOT NULL,
        PRIMARY KEY(EraType, AgendaTag),
        FOREIGN KEY (EraType) REFERENCES Eras(EraType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (AgendaTag) REFERENCES AgendaTags(AgendaTagType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "RandomAgendasForCivic" (
        "CivicType" TEXT NOT NULL,
        "NumAgendas" INTEGER NOT NULL DEFAULT 1,
        "VisibilityType" TEXT NOT NULL,
        PRIMARY KEY(CivicType),
        FOREIGN KEY (CivicType) REFERENCES Civics(CivicType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (VisibilityType) REFERENCES Visibilities(VisibilityType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "RandomAgendasInEra" (
        "EraType" TEXT NOT NULL,
        "NumAgendas" INTEGER NOT NULL DEFAULT 0,
        "VisibilityType" TEXT NOT NULL,
        PRIMARY KEY(EraType),
        FOREIGN KEY (EraType) REFERENCES Eras(EraType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (VisibilityType) REFERENCES Visibilities(VisibilityType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "RandomEvents" (
        "RandomEventType" TEXT NOT NULL,
        "Name" TEXT NOT NULL,
        "Description" TEXT NOT NULL,
        "LongDescription" TEXT,
        "EffectString" TEXT,
        "Severity" INTEGER NOT NULL DEFAULT -1,
        "NaturalWonder" TEXT,
        "IceLoss" INTEGER NOT NULL DEFAULT 0,
        "HaltsStormFertility" BOOLEAN NOT NULL CHECK (HaltsStormFertility IN (0,1)) DEFAULT 0,
        "HaltsFloodFertility" BOOLEAN NOT NULL CHECK (HaltsFloodFertility IN (0,1)) DEFAULT 0,
        "FertilityRemovalChance" INTEGER NOT NULL DEFAULT 0,
        "ClimateChangePoints" INTEGER NOT NULL DEFAULT 0,
        "ChanceIncreasePerDegree" INTEGER NOT NULL DEFAULT 0,
        "Hexes" INTEGER NOT NULL DEFAULT 0,
        "Movement" INTEGER NOT NULL DEFAULT 0,
        "Duration" INTEGER NOT NULL DEFAULT 0,
        "Spacing" INTEGER NOT NULL DEFAULT 0,
        "IconLarge" TEXT,
        "IconSmall" TEXT,
        "MinTurnAtRisk" INTEGER NOT NULL DEFAULT 0,
        "MitigatedYieldReduction" INTEGER NOT NULL DEFAULT 0,
        "EffectOperatorType" TEXT,
        "UnitTriggered" BOOLEAN NOT NULL CHECK (UnitTriggered IN (0,1)) DEFAULT 0,
        "Global" BOOLEAN NOT NULL CHECK (Global IN (0,1)) DEFAULT 0,
        "AvoidTerritory" BOOLEAN NOT NULL CHECK (AvoidTerritory IN (0,1)) DEFAULT 0,
        "TargetCities" BOOLEAN NOT NULL CHECK (TargetCities IN (0,1)) DEFAULT 0,
        PRIMARY KEY(RandomEventType),
        FOREIGN KEY (NaturalWonder) REFERENCES Features(FeatureType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "RandomEvent_Damages" (
        "RandomEventType" TEXT NOT NULL,
        "DamageType" TEXT NOT NULL,
        "Percentage" INTEGER NOT NULL DEFAULT 0,
        "MinHP" INTEGER NOT NULL DEFAULT 0,
        "MaxHP" INTEGER NOT NULL DEFAULT 0,
        "CoastalLowlandPercentage" INTEGER,
        "FalloutDuration" INTEGER NOT NULL DEFAULT 0,
        "ExtraRangePercentage" INTEGER NOT NULL DEFAULT 0,
        "MinTurn" INTEGER,
        "MaxTurn" INTEGER,
        PRIMARY KEY(RandomEventType, DamageType),
        FOREIGN KEY (RandomEventType) REFERENCES RandomEvents(RandomEventType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "RandomEvent_DamagedUnits" (
        "RandomEventType" TEXT NOT NULL,
        "UnitType" TEXT NOT NULL,
        PRIMARY KEY(RandomEventType, UnitType));

CREATE TABLE "RandomEvent_Features" (
        "RandomEventType" TEXT NOT NULL,
        "FeatureType" TEXT NOT NULL,
        PRIMARY KEY(RandomEventType, FeatureType));

CREATE TABLE "RandomEvent_Frequencies" (
        "RandomEventType" TEXT NOT NULL,
        "RealismSettingType" TEXT NOT NULL,
        "OccurrencesPerGame" REAL NOT NULL DEFAULT 0,
        PRIMARY KEY(RandomEventType, RealismSettingType),
        FOREIGN KEY (RandomEventType) REFERENCES RandomEvents(RandomEventType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (RealismSettingType) REFERENCES RealismSettings(RealismSettingType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "RandomEvent_Improvement_Placements" (
        "RandomEventType" TEXT NOT NULL,
        "ImprovementType" TEXT NOT NULL,
        PRIMARY KEY(RandomEventType, ImprovementType));

CREATE TABLE "RandomEvent_Notifications" (
        "RandomEventType" TEXT NOT NULL,
        "Summary" TEXT NOT NULL,
        "MinTurn" INTEGER,
        "MaxTurn" INTEGER,
        "Title" TEXT NOT NULL,
        "CompilationRadius" INTEGER,
        "NotificationType" TEXT NOT NULL,
        PRIMARY KEY(RandomEventType, Summary, Title, NotificationType));

CREATE TABLE "RandomEvent_PillagedBuildings" (
        "RandomEventType" TEXT NOT NULL,
        "BuildingType" TEXT NOT NULL,
        PRIMARY KEY(RandomEventType, BuildingType));

CREATE TABLE "RandomEvent_PillagedDistricts" (
        "RandomEventType" TEXT NOT NULL,
        "DistrictType" TEXT NOT NULL,
        PRIMARY KEY(RandomEventType, DistrictType));

CREATE TABLE "RandomEvent_PillagedImprovements" (
        "RandomEventType" TEXT NOT NULL,
        "ImprovementType" TEXT NOT NULL,
        PRIMARY KEY(RandomEventType, ImprovementType),
        FOREIGN KEY (RandomEventType) REFERENCES RandomEvents(RandomEventType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (ImprovementType) REFERENCES Improvements(ImprovementType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "RandomEvent_Presentation" (
        "RandomEventType" TEXT NOT NULL,
        "Animation" TEXT NOT NULL,
        "Sound" TEXT NOT NULL,
        "Callback" TEXT,
        "VFX" TEXT,
        "ForceShowVFX" TEXT,
        "MFX" TEXT,
        "SequenceType" TEXT,
        PRIMARY KEY(RandomEventType),
        FOREIGN KEY (RandomEventType) REFERENCES RandomEvents(RandomEventType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "RandomEvent_Terrains" (
        "RandomEventType" TEXT NOT NULL,
        "TerrainType" TEXT NOT NULL,
        PRIMARY KEY(RandomEventType, TerrainType),
        FOREIGN KEY (RandomEventType) REFERENCES RandomEvents(RandomEventType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (TerrainType) REFERENCES Terrains(TerrainType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "RandomEvent_Yields" (
        "RandomEventType" TEXT NOT NULL,
        "YieldType" TEXT NOT NULL,
        "FeatureType" TEXT NOT NULL,
        "Percentage" INTEGER NOT NULL DEFAULT 0,
        "ReplaceFeature" BOOLEAN NOT NULL CHECK (ReplaceFeature IN (0,1)) DEFAULT 0,
        "Amount" INTEGER NOT NULL DEFAULT 1,
        "Turn" INTEGER,
        PRIMARY KEY(RandomEventType, YieldType, FeatureType),
        FOREIGN KEY (RandomEventType) REFERENCES RandomEvents(RandomEventType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (YieldType) REFERENCES Yields(YieldType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (FeatureType) REFERENCES Features(FeatureType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "RealismSettings" (
        "RealismSettingType" TEXT NOT NULL,
        "Name" TEXT NOT NULL,
        "PercentVolcanoesActive" INTEGER NOT NULL DEFAULT 100,
        "ExtraRange" BOOLEAN NOT NULL CHECK (ExtraRange IN (0,1)) DEFAULT 0,
        "ClimateChangePoints" INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY(RealismSettingType));

CREATE TABLE "Resolutions" (
        "ResolutionType" TEXT NOT NULL,
        "TargetKind" TEXT NOT NULL,
        "Name" TEXT NOT NULL,
        "Effect1Description" TEXT,
        "Effect2Description" TEXT,
        "ValidationLua" TEXT,
        "AITargetChooser" TEXT,
        "AILuaTargetChooser" TEXT,
        "InjectionOnly" BOOLEAN NOT NULL CHECK (InjectionOnly IN (0,1)) DEFAULT 0,
        "EarliestEra" TEXT,
        "LatestEra" TEXT,
        PRIMARY KEY(ResolutionType),
        FOREIGN KEY (ResolutionType) REFERENCES Types(Type) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "ResolutionEffects" (
        "ResolutionEffectId" INTEGER NOT NULL,
        "ResolutionType" TEXT NOT NULL,
        "WhichEffect" INTEGER NOT NULL DEFAULT 1,
        "ModifierId" TEXT NOT NULL,
        PRIMARY KEY(ResolutionEffectId),
        FOREIGN KEY (ResolutionType) REFERENCES Resolutions(ResolutionType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "Resource_Consumption" (
        "ResourceType" TEXT NOT NULL,
        "Accumulate" BOOLEAN NOT NULL CHECK (Accumulate IN (0,1)) DEFAULT 0,
        "PowerProvided" INTEGER NOT NULL DEFAULT 0,
        "CO2perkWh" INTEGER NOT NULL DEFAULT 0,
        "BaseExtractionRate" INTEGER NOT NULL DEFAULT 0,
        "ImprovedExtractionRate" INTEGER NOT NULL DEFAULT 0,
        "ObsoleteTech" TEXT,
        "StockpileCap" INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY(ResourceType),
        FOREIGN KEY (ResourceType) REFERENCES Resources(ResourceType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (ObsoleteTech) REFERENCES Technologies(TechnologyType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "Route_ResourceCosts" (
        "RouteType" TEXT NOT NULL,
        "ResourceType" TEXT NOT NULL,
        "BuildWithUnitCost" INTEGER NOT NULL,
        PRIMARY KEY(RouteType, ResourceType),
        FOREIGN KEY (RouteType) REFERENCES Routes(RouteType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (ResourceType) REFERENCES Resources(ResourceType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "Routes_XP2" (
        "RouteType" TEXT NOT NULL,
        "BuildOnlyWithUnit" BOOLEAN NOT NULL CHECK (BuildOnlyWithUnit IN (0,1)) DEFAULT 0,
        "BuildWithUnitChargeCost" INTEGER NOT NULL,
        "PrereqTech" TEXT,
        PRIMARY KEY(RouteType),
        FOREIGN KEY (RouteType) REFERENCES Routes(RouteType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (PrereqTech) REFERENCES Technologies(TechnologyType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "SecretSocieties" (
        "SecretSocietyType" TEXT NOT NULL,
        "Name" LocalizedText NOT NULL,
        "Description" LocalizedText NOT NULL,
        "DiscoveryText" LocalizedText NOT NULL,
        "MembershipText" LocalizedText NOT NULL,
        "GovernorType" TEXT,
        "DiscoverAtBarbarianCampBaseChance" INTEGER NOT NULL DEFAULT 0,
        "DiscoverAtCityStateBaseChance" INTEGER NOT NULL DEFAULT 0,
        "DiscoverAtGoodyHutBaseChance" INTEGER NOT NULL DEFAULT 0,
        "DiscoverAtNaturalWonderBaseChance" INTEGER NOT NULL DEFAULT 0,
        "SmallIcon" TEXT NOT NULL,
        "IconString" TEXT NOT NULL,
        PRIMARY KEY(SecretSocietyType),
        FOREIGN KEY (GovernorType) REFERENCES Governors(GovernorType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "StartEras_XP2" (
        "EraType" TEXT,
        "DiploVP" INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY(EraType),
        FOREIGN KEY (EraType) REFERENCES StartEras(EraType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "Unit_RockbandResults_XP2" (
        "ResultType" TEXT NOT NULL,
        "Name" TEXT NOT NULL,
        "Description" TEXT NOT NULL,
        "AlbumSales" INTEGER NOT NULL DEFAULT 0,
        "TourismBomb" INTEGER NOT NULL DEFAULT 0,
        "ExtraPromotion" BOOLEAN NOT NULL CHECK (ExtraPromotion IN (0,1)) DEFAULT 0,
        "Dies" BOOLEAN NOT NULL CHECK (Dies IN (0,1)) DEFAULT 0,
        "GainsLevel" BOOLEAN NOT NULL CHECK (GainsLevel IN (0,1)) DEFAULT 0,
        "BaseProbability" INTEGER NOT NULL DEFAULT 0,
        "PerformanceStars" INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY(ResultType));

CREATE TABLE "Units_XP2" (
        "UnitType" TEXT NOT NULL,
        "ResourceMaintenanceAmount" INTEGER NOT NULL DEFAULT 0,
        "ResourceCost" INTEGER NOT NULL DEFAULT 0,
        "ResourceMaintenanceType" TEXT,
        "TourismBomb" INTEGER NOT NULL DEFAULT 0,
        "CanEarnExperience" BOOLEAN NOT NULL CHECK (CanEarnExperience IN (0,1)) DEFAULT 1,
        "TourismBombPossible" BOOLEAN NOT NULL CHECK (TourismBombPossible IN (0,1)) DEFAULT 0,
        "CanFormMilitaryFormation" BOOLEAN NOT NULL CHECK (CanFormMilitaryFormation IN (0,1)) DEFAULT 1,
        "MajorCivOnly" BOOLEAN NOT NULL CHECK (MajorCivOnly IN (0,1)) DEFAULT 0,
        "CanCauseDisasters" BOOLEAN NOT NULL CHECK (CanCauseDisasters IN (0,1)) DEFAULT 0,
        "CanSacrificeUnits" BOOLEAN NOT NULL CHECK (CanSacrificeUnits IN (0,1)) DEFAULT 0,
        PRIMARY KEY(UnitType),
        FOREIGN KEY (UnitType) REFERENCES Units(UnitType) ON DELETE CASCADE ON UPDATE CASCADE,
        FOREIGN KEY (ResourceMaintenanceType) REFERENCES Resources(ResourceType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "UnitOperations_XP2" (
        "OperationType" TEXT NOT NULL,
        "IsDisabled" BOOLEAN NOT NULL CHECK (IsDisabled IN (0,1)) DEFAULT 0,
        "CO2Production" INTEGER NOT NULL DEFAULT 0,
        PRIMARY KEY(OperationType));

CREATE TABLE "UnitRetreats_XP1" (
        "UnitRetreatType" TEXT NOT NULL,
        "BuildingType" TEXT,
        "UnitType" TEXT NOT NULL,
        "ImprovementType" TEXT,
        PRIMARY KEY(UnitRetreatType, UnitType));

CREATE TABLE "Visibilities_XP2" (
        "VisibilityType" TEXT NOT NULL,
        "EspionageViewCapital" BOOLEAN NOT NULL CHECK (EspionageViewCapital IN (0,1)) DEFAULT 0,
        "EspionageViewAll" BOOLEAN NOT NULL CHECK (EspionageViewAll IN (0,1)) DEFAULT 0,
        PRIMARY KEY(VisibilityType),
        FOREIGN KEY (VisibilityType) REFERENCES Visibilities(VisibilityType) ON DELETE CASCADE ON UPDATE CASCADE);

CREATE TABLE "VotingBlockers" (
        "VotingBlockerType" TEXT NOT NULL,
        "NoUpvote" BOOLEAN NOT NULL CHECK (NoUpvote IN (0,1)) DEFAULT 0,
        "NoDownvote" BOOLEAN NOT NULL CHECK (NoDownvote IN (0,1)) DEFAULT 0,
        "Description" TEXT NOT NULL,
        PRIMARY KEY(VotingBlockerType),
        FOREIGN KEY (VotingBlockerType) REFERENCES Types(Type) ON DELETE CASCADE ON UPDATE CASCADE);


-- Navigation Properties (if any)
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("AgendaTags", "RandomAgendaCollection", "Agendas", 1,"SELECT T1.rowid from Agendas as T1 inner join RandomAgendas_XP2 as T2 on T2.AgendaType = T1.AgendaType inner join AgendaTags as T3 on T3.AgendaTagType = T2.AgendaTag where T3.rowid = ? ORDER BY T1.rowid ASC");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("Alliances", "ActionCollection", "DiplomaticActions", 1,"SELECT T1.rowid from DiplomaticActions as T1 inner join DiplomaticActions_XP1 as T2 on T2.DiplomaticActionType = T1.DiplomaticActionType inner join Alliances as T3 on T3.AllianceType = T2.AllianceType where T3.rowid = ? ORDER BY T1.rowid ASC");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("Building_ResourceCosts", "BuildingReference", "Buildings", 0,"SELECT T1.rowid from Buildings as T1 inner join Building_ResourceCosts as T2 on T2.BuildingType = T1.BuildingType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("Building_ResourceCosts", "ResourceReference", "Resources", 0,"SELECT T1.rowid from Resources as T1 inner join Building_ResourceCosts as T2 on T2.ResourceType = T1.ResourceType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("Building_TourismBombs_XP2", "BuildingReference", "Buildings", 0,"SELECT T1.rowid from Buildings as T1 inner join Building_TourismBombs_XP2 as T2 on T2.BuildingType = T1.BuildingType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("Buildings_XP2", "BuildingReference", "Buildings", 0,"SELECT T1.rowid from Buildings as T1 inner join Buildings_XP2 as T2 on T2.BuildingType = T1.BuildingType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("Buildings_XP2", "ResourceTypeConvertedToPowerReference", "Resources", 0,"SELECT T1.rowid from Resources as T1 inner join Buildings_XP2 as T2 on T2.ResourceTypeConvertedToPower = T1.ResourceType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("Building_YieldChangesBonusWithPower", "YieldReference", "Yields", 0,"SELECT T1.rowid from Yields as T1 inner join Building_YieldChangesBonusWithPower as T2 on T2.YieldType = T1.YieldType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("CommemorationTypes", "CommemorationCollection", "CommemorationModifiers", 1,"SELECT T1.rowid from CommemorationModifiers as T1 inner join CommemorationTypes as T2 on T2.CommemorationType = T1.CommemorationType where T2.rowid = ? ORDER BY T1.rowid ASC");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("CommemorationTypes", "ComplimentCollection", "ComplimentModifiers", 1,"SELECT T1.rowid from ComplimentModifiers as T1 inner join CommemorationTypes as T2 on T2.CommemorationType = T1.CommemorationType where T2.rowid = ? ORDER BY T1.rowid ASC");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("CommemorationTypes", "MaximumGameEraReference", "Eras", 0,"SELECT T1.rowid from Eras as T1 inner join CommemorationTypes as T2 on T2.MaximumGameEra = T1.EraType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("CommemorationTypes", "MinimumGameEraReference", "Eras", 0,"SELECT T1.rowid from Eras as T1 inner join CommemorationTypes as T2 on T2.MinimumGameEra = T1.EraType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("CongressAiChanges", "DiscussionReference", "Discussions", 0,"SELECT T1.rowid from Discussions as T1 inner join CongressAiChanges as T2 on T2.DiscussionType = T1.DiscussionType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("CongressAiChanges", "PseudoYieldReference", "PseudoYields", 0,"SELECT T1.rowid from PseudoYields as T1 inner join CongressAiChanges as T2 on T2.PseudoYieldType = T1.PseudoYieldType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("CongressAiChanges", "ResolutionReference", "Resolutions", 0,"SELECT T1.rowid from Resolutions as T1 inner join CongressAiChanges as T2 on T2.ResolutionType = T1.ResolutionType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("CongressAiChanges", "YieldReference", "Yields", 0,"SELECT T1.rowid from Yields as T1 inner join CongressAiChanges as T2 on T2.YieldType = T1.YieldType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("DiplomaticActions_XP1", "AllianceReference", "Alliances", 0,"SELECT T1.rowid from Alliances as T1 inner join DiplomaticActions_XP1 as T2 on T2.AllianceType = T1.AllianceType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("DiplomaticActions_XP1", "RequiresGoldenAgeCommemorationTypeReference", "CommemorationTypes", 0,"SELECT T1.rowid from CommemorationTypes as T1 inner join DiplomaticActions_XP1 as T2 on T2.RequiresGoldenAgeCommemorationType = T1.CommemorationType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("DiplomaticVisibilitySources_XP1", "TradePostTraitReference", "Traits", 0,"SELECT T1.rowid from Traits as T1 inner join DiplomaticVisibilitySources_XP1 as T2 on T2.TradePostTrait = T1.TraitType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("Discussions", "AiChangeCollectionReference", "CongressAiChanges", 1,"SELECT T1.rowid from CongressAiChanges as T1 inner join Discussions as T2 on T2.DiscussionType = T1.DiscussionType where T2.rowid = ? ORDER BY T1.rowid ASC");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("Discussions", "EmergencyReference", "EmergencyAlliances", 0,"SELECT T1.rowid from EmergencyAlliances as T1 inner join Emergencies_XP2 as T2 on T2.EmergencyType = T1.EmergencyType inner join Discussions as T3 on T3.EmergencyType = T2.EmergencyType where T3.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("EmergencyAlliances", "Effects", "EmergencyBuffs", 1,"SELECT T1.rowid from EmergencyBuffs as T1 inner join EmergencyAlliances as T2 on T2.EmergencyType = T1.EmergencyType where T2.rowid = ? ORDER BY T1.rowid ASC");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("EmergencyAlliances", "EmergencyTextBlock", "EmergencyTexts", 0,"SELECT T1.rowid from EmergencyTexts as T1 inner join EmergencyAlliances as T2 on T2.EmergencyText = T1.Type where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("EmergencyAlliances", "GoalTextBlock", "EmergencyGoalTexts", 0,"SELECT T1.rowid from EmergencyGoalTexts as T1 inner join EmergencyAlliances as T2 on T2.GoalText = T1.GoalType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("EmergencyAlliances", "Rewards", "EmergencyRewards", 1,"SELECT T1.rowid from EmergencyRewards as T1 inner join EmergencyAlliances as T2 on T2.EmergencyType = T1.EmergencyType where T2.rowid = ? ORDER BY T1.rowid ASC");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("Emergencies_XP2", "DiscussionCollectionReference", "Discussions", 1,"SELECT T1.rowid from Discussions as T1 inner join Emergencies_XP2 as T2 on T2.EmergencyType = T1.EmergencyType where T2.rowid = ? ORDER BY T1.rowid ASC");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("Emergencies_XP2", "EmergencyReference", "EmergencyAlliances", 0,"SELECT T1.rowid from EmergencyAlliances as T1 inner join Emergencies_XP2 as T2 on T2.EmergencyType = T1.EmergencyType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("Emergencies_XP2", "ScoreSources", "EmergencyScoreSources", 1,"SELECT T1.rowid from EmergencyScoreSources as T1 inner join Emergencies_XP2 as T2 on T2.EmergencyType = T1.EmergencyType where T2.rowid = ? ORDER BY T1.rowid ASC");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("EmergencyBuffs", "EmergencyRef", "EmergencyAlliances", 0,"SELECT T1.rowid from EmergencyAlliances as T1 inner join EmergencyBuffs as T2 on T2.EmergencyType = T1.EmergencyType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("Governors", "PromotionCollection", "GovernorPromotions", 1,"SELECT T1.rowid from GovernorPromotions as T1 inner join GovernorPromotionSets as T2 on T2.GovernorPromotion = T1.GovernorPromotionType inner join Governors as T3 on T3.GovernorType = T2.GovernorType where T3.rowid = ? ORDER BY T1.rowid ASC");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("Governors", "SecretSocietyCollection", "SecretSocieties", 1,"SELECT T1.rowid from SecretSocieties as T1 inner join Governors as T2 on T2.GovernorType = T1.GovernorType where T2.rowid = ? ORDER BY T1.rowid ASC");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("GovernorPromotions", "GovernorCollection", "Governors", 1,"SELECT T1.rowid from Governors as T1 inner join GovernorPromotionSets as T2 on T2.GovernorType = T1.GovernorType inner join GovernorPromotions as T3 on T3.GovernorPromotionType = T2.GovernorPromotion where T3.rowid = ? ORDER BY T1.rowid ASC");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("GovernorPromotions", "PrereqGovernorPromotions", "GovernorPromotions", 1,"SELECT T1.rowid from GovernorPromotions as T1 inner join GovernorPromotionPrereqs as T2 on T2.PrereqGovernorPromotion = T1.GovernorPromotionType inner join GovernorPromotions as T3 on T3.GovernorPromotionType = T2.GovernorPromotionType where T3.rowid = ? ORDER BY T1.rowid ASC");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("GovernorPromotionConditions", "EarliestGameEraReference", "Eras", 0,"SELECT T1.rowid from Eras as T1 inner join GovernorPromotionConditions as T2 on T2.EarliestGameEra = T1.EraType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("GovernorPromotionModifiers", "GovernorPromotionReference", "GovernorPromotions", 0,"SELECT T1.rowid from GovernorPromotions as T1 inner join GovernorPromotionModifiers as T2 on T2.GovernorPromotionType = T1.GovernorPromotionType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("GovernorReplaces", "BaseGovernorReference", "Governors", 0,"SELECT T1.rowid from Governors as T1 inner join GovernorReplaces as T2 on T2.ReplacesGovernorType = T1.GovernorType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("GovernorReplaces", "ReplacementGovernorReference", "Governors", 0,"SELECT T1.rowid from Governors as T1 inner join GovernorReplaces as T2 on T2.UniqueGovernorType = T1.GovernorType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("Moments", "CommemorationReference", "CommemorationTypes", 0,"SELECT T1.rowid from CommemorationTypes as T1 inner join Moments as T2 on T2.CommemorationType = T1.CommemorationType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("Moments", "MaximumGameEraReference", "Eras", 0,"SELECT T1.rowid from Eras as T1 inner join Moments as T2 on T2.MaximumGameEra = T1.EraType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("Moments", "MinimumGameEraReference", "Eras", 0,"SELECT T1.rowid from Eras as T1 inner join Moments as T2 on T2.MinimumGameEra = T1.EraType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("Moments", "ObsoleteEraReference", "Eras", 0,"SELECT T1.rowid from Eras as T1 inner join Moments as T2 on T2.ObsoleteEra = T1.EraType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("NamedDeserts", "DesertCivilizations", "Civilizations", 1,"SELECT T1.rowid from Civilizations as T1 inner join NamedDesertCivilizations as T2 on T2.CivilizationType = T1.CivilizationType inner join NamedDeserts as T3 on T3.NamedDesertType = T2.NamedDesertType where T3.rowid = ? ORDER BY T1.rowid ASC");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("NamedDesertCivilizations", "NamedDesertReference", "NamedDeserts", 0,"SELECT T1.rowid from NamedDeserts as T1 inner join NamedDesertCivilizations as T2 on T2.NamedDesertType = T1.NamedDesertType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("NamedLakes", "LakeCivilizations", "Civilizations", 1,"SELECT T1.rowid from Civilizations as T1 inner join NamedLakeCivilizations as T2 on T2.CivilizationType = T1.CivilizationType inner join NamedLakes as T3 on T3.NamedLakeType = T2.NamedLakeType where T3.rowid = ? ORDER BY T1.rowid ASC");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("NamedLakeCivilizations", "NamedLakeReference", "NamedLakes", 0,"SELECT T1.rowid from NamedLakes as T1 inner join NamedLakeCivilizations as T2 on T2.NamedLakeType = T1.NamedLakeType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("NamedMountains", "MountainCivilizations", "Civilizations", 1,"SELECT T1.rowid from Civilizations as T1 inner join NamedMountainCivilizations as T2 on T2.CivilizationType = T1.CivilizationType inner join NamedMountains as T3 on T3.NamedMountainType = T2.NamedMountainType where T3.rowid = ? ORDER BY T1.rowid ASC");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("NamedMountainCivilizations", "NamedMountainReference", "NamedMountains", 0,"SELECT T1.rowid from NamedMountains as T1 inner join NamedMountainCivilizations as T2 on T2.NamedMountainType = T1.NamedMountainType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("NamedOceans", "OceanCivilizations", "Civilizations", 1,"SELECT T1.rowid from Civilizations as T1 inner join NamedOceanCivilizations as T2 on T2.CivilizationType = T1.CivilizationType inner join NamedOceans as T3 on T3.NamedOceanType = T2.NamedOceanType where T3.rowid = ? ORDER BY T1.rowid ASC");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("NamedOceanCivilizations", "NamedOceanReference", "NamedOceans", 0,"SELECT T1.rowid from NamedOceans as T1 inner join NamedOceanCivilizations as T2 on T2.NamedOceanType = T1.NamedOceanType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("NamedRivers", "RiverCivilizations", "Civilizations", 1,"SELECT T1.rowid from Civilizations as T1 inner join NamedRiverCivilizations as T2 on T2.CivilizationType = T1.CivilizationType inner join NamedRivers as T3 on T3.NamedRiverType = T2.NamedRiverType where T3.rowid = ? ORDER BY T1.rowid ASC");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("NamedRiverCivilizations", "NamedRiverReference", "NamedRivers", 0,"SELECT T1.rowid from NamedRivers as T1 inner join NamedRiverCivilizations as T2 on T2.NamedRiverType = T1.NamedRiverType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("NamedSeas", "SeaCivilizations", "Civilizations", 1,"SELECT T1.rowid from Civilizations as T1 inner join NamedSeaCivilizations as T2 on T2.CivilizationType = T1.CivilizationType inner join NamedSeas as T3 on T3.NamedSeaType = T2.NamedSeaType where T3.rowid = ? ORDER BY T1.rowid ASC");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("NamedSeaCivilizations", "NamedSeaReference", "NamedSeas", 0,"SELECT T1.rowid from NamedSeas as T1 inner join NamedSeaCivilizations as T2 on T2.NamedSeaType = T1.NamedSeaType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("NamedVolcanoes", "VolcanoCivilizations", "Civilizations", 1,"SELECT T1.rowid from Civilizations as T1 inner join NamedVolcanoCivilizations as T2 on T2.CivilizationType = T1.CivilizationType inner join NamedVolcanoes as T3 on T3.NamedVolcanoType = T2.NamedVolcanoType where T3.rowid = ? ORDER BY T1.rowid ASC");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("NamedVolcanoCivilizations", "NamedVolcanoReference", "NamedVolcanoes", 0,"SELECT T1.rowid from NamedVolcanoes as T1 inner join NamedVolcanoCivilizations as T2 on T2.NamedVolcanoType = T1.NamedVolcanoType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("Policies_XP1", "MaximumGameEraReference", "Eras", 0,"SELECT T1.rowid from Eras as T1 inner join Policies_XP1 as T2 on T2.MaximumGameEra = T1.EraType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("Policies_XP1", "MinimumGameEraReference", "Eras", 0,"SELECT T1.rowid from Eras as T1 inner join Policies_XP1 as T2 on T2.MinimumGameEra = T1.EraType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("Project_BuildingCosts", "ConsumedBuildingReference", "Buildings", 0,"SELECT T1.rowid from Buildings as T1 inner join Project_BuildingCosts as T2 on T2.ConsumedBuildingType = T1.BuildingType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("Project_BuildingCosts", "ProjectReference", "Projects", 0,"SELECT T1.rowid from Projects as T1 inner join Project_BuildingCosts as T2 on T2.ProjectType = T1.ProjectType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("Project_ResourceCosts", "ProjectReference", "Projects", 0,"SELECT T1.rowid from Projects as T1 inner join Project_ResourceCosts as T2 on T2.ProjectType = T1.ProjectType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("Project_ResourceCosts", "ResourceReference", "Resources", 0,"SELECT T1.rowid from Resources as T1 inner join Project_ResourceCosts as T2 on T2.ResourceType = T1.ResourceType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("Projects_XP2", "BuildingCostCollectionReference", "Project_BuildingCosts", 1,"SELECT T1.rowid from Project_BuildingCosts as T1 inner join Projects_XP2 as T2 on T2.ProjectType = T1.ProjectType where T2.rowid = ? ORDER BY T1.rowid ASC");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("Projects_XP2", "CreateBuildingReference", "Buildings", 0,"SELECT T1.rowid from Buildings as T1 inner join Projects_XP2 as T2 on T2.CreateBuilding = T1.BuildingType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("Projects_XP2", "EmergencyScoreCollectionReference", "EmergencyScoreSources", 1,"SELECT T1.rowid from EmergencyScoreSources as T1 inner join Projects_XP2 as T2 on T2.ProjectType = T1.FromProject where T2.rowid = ? ORDER BY T1.rowid ASC");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("Projects_XP2", "RequiredBuildingReference", "Buildings", 0,"SELECT T1.rowid from Buildings as T1 inner join Projects_XP2 as T2 on T2.RequiredBuilding = T1.BuildingType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("RandomAgendas_XP2", "AgendaReference", "Agendas", 0,"SELECT T1.rowid from Agendas as T1 inner join RandomAgendas_XP2 as T2 on T2.AgendaType = T1.AgendaType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("RandomAgendas_XP2", "RandomAgendaCollection", "RandomAgendas", 1,"SELECT T1.rowid from RandomAgendas as T1 inner join Agendas as T2 on T2.AgendaType = T1.AgendaType inner join RandomAgendas_XP2 as T3 on T3.AgendaType = T2.AgendaType where T3.rowid = ? ORDER BY T1.rowid ASC");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("RandomAgendasForCivic", "AgendaTagCollection", "AgendaTags", 1,"SELECT T1.rowid from AgendaTags as T1 inner join RandomAgendaCivicTags as T2 on T2.AgendaTag = T1.AgendaTagType inner join Civics as T3 on T3.CivicType = T2.CivicType inner join RandomAgendasForCivic as T4 on T4.CivicType = T3.CivicType where T4.rowid = ? ORDER BY T1.rowid ASC");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("RandomAgendasForCivic", "CivicReference", "Civics", 0,"SELECT T1.rowid from Civics as T1 inner join RandomAgendasForCivic as T2 on T2.CivicType = T1.CivicType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("RandomAgendasForCivic", "VisibilityReference", "Visibilities", 0,"SELECT T1.rowid from Visibilities as T1 inner join RandomAgendasForCivic as T2 on T2.VisibilityType = T1.VisibilityType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("RandomAgendasInEra", "AgendaTagCollection", "AgendaTags", 1,"SELECT T1.rowid from AgendaTags as T1 inner join RandomAgendaEraTags as T2 on T2.AgendaTag = T1.AgendaTagType inner join Eras as T3 on T3.EraType = T2.EraType inner join RandomAgendasInEra as T4 on T4.EraType = T3.EraType where T4.rowid = ? ORDER BY T1.rowid ASC");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("RandomAgendasInEra", "EraReference", "Eras", 0,"SELECT T1.rowid from Eras as T1 inner join RandomAgendasInEra as T2 on T2.EraType = T1.EraType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("RandomAgendasInEra", "VisibilityReference", "Visibilities", 0,"SELECT T1.rowid from Visibilities as T1 inner join RandomAgendasInEra as T2 on T2.VisibilityType = T1.VisibilityType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("RandomEvents", "DamageCollection", "RandomEvent_Damages", 1,"SELECT T1.rowid from RandomEvent_Damages as T1 inner join RandomEvents as T2 on T2.RandomEventType = T1.RandomEventType where T2.rowid = ? ORDER BY T1.rowid ASC");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("RandomEvents", "FrequencyCollection", "RandomEvent_Frequencies", 1,"SELECT T1.rowid from RandomEvent_Frequencies as T1 inner join RandomEvents as T2 on T2.RandomEventType = T1.RandomEventType where T2.rowid = ? ORDER BY T1.rowid ASC");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("RandomEvents", "NaturalWonderReference", "Features", 0,"SELECT T1.rowid from Features as T1 inner join RandomEvents as T2 on T2.NaturalWonder = T1.FeatureType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("RandomEvents", "PillagedImprovementCollection", "RandomEvent_PillagedImprovements", 1,"SELECT T1.rowid from RandomEvent_PillagedImprovements as T1 inner join RandomEvents as T2 on T2.RandomEventType = T1.RandomEventType where T2.rowid = ? ORDER BY T1.rowid ASC");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("RandomEvents", "TerrainCollection", "RandomEvent_Terrains", 1,"SELECT T1.rowid from RandomEvent_Terrains as T1 inner join RandomEvents as T2 on T2.RandomEventType = T1.RandomEventType where T2.rowid = ? ORDER BY T1.rowid ASC");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("RandomEvents", "YieldCollection", "RandomEvent_Yields", 1,"SELECT T1.rowid from RandomEvent_Yields as T1 inner join RandomEvents as T2 on T2.RandomEventType = T1.RandomEventType where T2.rowid = ? ORDER BY T1.rowid ASC");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("RandomEvent_Damages", "RandomEventReference", "RandomEvents", 0,"SELECT T1.rowid from RandomEvents as T1 inner join RandomEvent_Damages as T2 on T2.RandomEventType = T1.RandomEventType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("Resolutions", "AiChangeCollectionReference", "CongressAiChanges", 1,"SELECT T1.rowid from CongressAiChanges as T1 inner join Resolutions as T2 on T2.ResolutionType = T1.ResolutionType where T2.rowid = ? ORDER BY T1.rowid ASC");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("Resolutions", "Effects", "ResolutionEffects", 1,"SELECT T1.rowid from ResolutionEffects as T1 inner join Resolutions as T2 on T2.ResolutionType = T1.ResolutionType where T2.rowid = ? ORDER BY T1.rowid ASC");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("Resource_Consumption", "ResourceReference", "Resources", 0,"SELECT T1.rowid from Resources as T1 inner join Resource_Consumption as T2 on T2.ResourceType = T1.ResourceType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("Resource_Consumption", "TechnologyReference", "Technologies", 0,"SELECT T1.rowid from Technologies as T1 inner join Resource_Consumption as T2 on T2.ObsoleteTech = T1.TechnologyType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("Route_ResourceCosts", "ResourceReference", "Resources", 0,"SELECT T1.rowid from Resources as T1 inner join Route_ResourceCosts as T2 on T2.ResourceType = T1.ResourceType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("Route_ResourceCosts", "RouteReference", "Routes", 0,"SELECT T1.rowid from Routes as T1 inner join Route_ResourceCosts as T2 on T2.RouteType = T1.RouteType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("Routes_XP2", "PrereqTechReference", "Technologies", 0,"SELECT T1.rowid from Technologies as T1 inner join Routes_XP2 as T2 on T2.PrereqTech = T1.TechnologyType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("Routes_XP2", "RouteReference", "Routes", 0,"SELECT T1.rowid from Routes as T1 inner join Routes_XP2 as T2 on T2.RouteType = T1.RouteType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("SecretSocieties", "GovernorReference", "Governors", 0,"SELECT T1.rowid from Governors as T1 inner join SecretSocieties as T2 on T2.GovernorType = T1.GovernorType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
INSERT INTO NavigationProperties("BaseTable", "PropertyName", "TargetTable", "IsCollection", "Query") VALUES("Units_XP2", "ResourceReference", "Resources", 0,"SELECT T1.rowid from Resources as T1 inner join Units_XP2 as T2 on T2.ResourceMaintenanceType = T1.ResourceType where T2.rowid = ? ORDER BY T1.rowid ASC LIMIT 1");
