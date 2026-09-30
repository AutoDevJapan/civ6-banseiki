-- 時代名を出す EraProgressPanel（Gathering Storm の Additions）へ家固有名の表示切替を取り付ける。
-- このコンテキストは ToolTipHelper の連鎖にもワイルドカード include にも乗らないため ReplaceUIScript で差し替え、元のファイルをそのまま読み込む。
-- 読んだ後に JAC_EraProgressPanel_*.lua をワイルドカードで読む（他の MOD がこの画面へ足す差分。戦国パックの時代スコア：
-- poc/JAC_PoC_SengokuTrees/UI/JAC_EraProgressPanel_Sengoku.lua）。ワイルドカード include の公式の例は
-- Civilopedia/CivilopediaScreen.lua:13 の include("CivilopediaPage_", true)。
include("JAC_HouseNames")
JAC_InstallHouseNames("EraProgressPanel")
include("EraProgressPanel")
include("JAC_EraProgressPanel_", true)
