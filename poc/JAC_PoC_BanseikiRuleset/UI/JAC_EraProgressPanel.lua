-- 時代名を出す EraProgressPanel（Gathering Storm の Additions）へ家固有名の表示切替を取り付ける。
-- このコンテキストは ToolTipHelper の連鎖にもワイルドカード include にも乗らないため ReplaceUIScript で差し替え、元のファイルをそのまま読み込む。
include("JAC_HouseNames")
JAC_InstallHouseNames("EraProgressPanel")
include("EraProgressPanel")
