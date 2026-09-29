-- 家固有名の表示切替（JAC_HouseNames.lua）をシヴィロペディアへ取り付ける。
-- CivilopediaScreen.lua:13 の include("CivilopediaPage_", true) で読まれる。ページ定義は含まない。
include("JAC_HouseNames")
JAC_InstallHouseNames(ContextPtr and ContextPtr:GetID() or "Civilopedia")
