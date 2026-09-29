-- 時代の終わりの予告（NewTurnPanel_Expansion1.lua:19 が時代名を出す）へ家固有名の表示切替を取り付ける。
-- Gathering Storm 自身が NewTurnPanel を ReplaceUIScript で NewTurnPanel_Expansion1 に差し替えているので、その差し替え先をそのまま読み込む
-- （NewTurnPanel_Expansion1.lua:7 が元の NewTurnPanel を読む）。
include("JAC_HouseNames")
JAC_InstallHouseNames("NewTurnPanel")
include("NewTurnPanel_Expansion1")
