-- 時代名を出す EraCompletePopup（Gathering Storm の Additions）へ家固有名の表示切替を取り付ける。
-- このコンテキストは ToolTipHelper の連鎖にもワイルドカード include にも乗らないため ReplaceUIScript で差し替え、元のファイルをそのまま読み込む。
-- 読んだ後に JAC_EraCompletePopup_*.lua をワイルドカードで読む（他の MOD がこの画面へ足す差分。戦国パックでは公式の
-- 画面を出さない：poc/JAC_PoC_SengokuTrees/UI/JAC_EraCompletePopup_Sengoku.lua）。JAC_EraProgressPanel.lua と同じ形。
include("JAC_HouseNames")
JAC_InstallHouseNames("EraCompletePopup")
include("EraCompletePopup")
include("JAC_EraCompletePopup_", true)
