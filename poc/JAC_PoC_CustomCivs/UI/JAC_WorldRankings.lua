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
	local target = Game:GetProperty("JAC_POC_CUSTOM_CIVS_RENAMED_NAME");
	if not target then return; end
	local oldLabel = Locale.Lookup("LOC_WORLD_RANKINGS_FIRST_PLACE_OTHER_SIMPLE",
		Locale.Lookup("LOC_CIVILIZATION_JAC_TAKEDA_NAME"));
	if instance.VictoryLeading:GetText() == oldLabel then
		local newLabel = Locale.Lookup("LOC_WORLD_RANKINGS_FIRST_PLACE_OTHER_SIMPLE", Locale.Lookup(target));
		instance.VictoryLeading:SetText(newLabel);
		if not loggedOverviewRename then
			loggedOverviewRename = true;
			print("[JAC_POC] worldRankingLabel result=applied displayed=" .. newLabel);
		end
	end
end
