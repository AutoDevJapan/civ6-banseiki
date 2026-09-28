-- 外交画面の表示名を上書きする。ReplaceUIScript で差し替え、GS 版の DiplomacyActionView_Expansion2 をそのまま読み込む。
include("JAC_CivNameOverride");
JAC_InstallCivNameOverride("DiplomacyActionView");
include("DiplomacyActionView_Expansion2");
