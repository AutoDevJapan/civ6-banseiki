-- 閃きポップアップ（技術・社会制度名を出す）へ家固有名の表示切替を取り付ける。
-- BoostUnlockedPopup.lua は SupportFunctions しか読まず、上の include の連鎖に乗らないため ReplaceUIScript で差し替え、元のファイルをそのまま読み込む。
include("JAC_HouseNames")
JAC_InstallHouseNames("BoostUnlockedPopup")
include("BoostUnlockedPopup")
