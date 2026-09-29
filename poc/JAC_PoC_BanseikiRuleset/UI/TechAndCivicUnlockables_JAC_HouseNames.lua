-- 家固有名の表示切替（JAC_HouseNames.lua）を、ToolTipHelper と Civ6Common を読み込むコンテキストへ取り付ける。
-- TechAndCivicUnlockables.lua の末尾の include("TechAndCivicUnlockables_", true) で読まれる。
-- ToolTipHelper.lua:12 が TechAndCivicUnlockables を読み、Civ6Common.lua:5 が ToolTipHelper を読むため、
-- 技術ツリー、社会制度ツリー、リサーチ／社会制度チューザー、ワールドトラッカー、研究完了ポップアップ、通知パネルなどに届く。
-- 公式の注意に従い、ここでは TechAndCivicUnlockables を include しない。
include("JAC_HouseNames")
JAC_InstallHouseNames(ContextPtr and ContextPtr:GetID() or "TechAndCivicUnlockables")
