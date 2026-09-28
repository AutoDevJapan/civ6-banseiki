-- 都市バナーの表示名を上書きする。GS の CityBannerManager.lua 末尾の include("CityBannerManager_", true) で読まれる。
-- 公式の注意に従い、ここでは CityBannerManager を include しない。
include("JAC_CivNameOverride");
JAC_InstallCivNameOverride("CityBannerManager");
