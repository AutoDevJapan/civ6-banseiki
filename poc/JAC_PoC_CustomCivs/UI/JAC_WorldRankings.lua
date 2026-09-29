-- ワールドランキングの表示名を上書きする。ReplaceUIScript で差し替え、GS 版の WorldRankings_Expansion2 をそのまま読み込む。
include("JAC_CivNameOverride");
JAC_InstallCivNameOverride("WorldRankings");
include("WorldRankings_Expansion2");

-- The overview leader label reads GameInfo.Civilizations[...].Name directly,
-- bypassing the PlayerConfiguration getters used by the rest of this screen.
local BasePopulateOverallInstance = PopulateOverallInstance;
local loggedOverviewRename = false;
function PopulateOverallInstance(instance, victoryType, typeText)
	BasePopulateOverallInstance(instance, victoryType, typeText);
	local shown = instance.VictoryLeading:GetText();
	for _, player in ipairs(PlayerManager.GetAliveMajors()) do
		local civType = PlayerConfigurations[player:GetID()]:GetCivilizationTypeName();
		local target = JAC_GetCivNameTarget(civType);
		local civ = target and GameInfo.Civilizations[civType];
		if civ and shown == Locale.Lookup("LOC_WORLD_RANKINGS_FIRST_PLACE_OTHER_SIMPLE", Locale.Lookup(civ.Name)) then
			local newLabel = Locale.Lookup("LOC_WORLD_RANKINGS_FIRST_PLACE_OTHER_SIMPLE", Locale.Lookup(target));
			instance.VictoryLeading:SetText(newLabel);
			if not loggedOverviewRename then
				loggedOverviewRename = true;
				print("[JAC_POC] worldRankingLabel result=applied displayed=" .. newLabel);
			end
			return;
		end
	end
end
