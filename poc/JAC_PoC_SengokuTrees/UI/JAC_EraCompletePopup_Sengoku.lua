-- 公式の新しい時代の画面を出さない。戦国パックでは時代＝元号で（オーナー判断 T、documents/poc/era-equals-reign.md）、
-- 時代が変わるターンに改元の画面（UI/JAC_SengokuReignPopup.lua）が出る。
-- poc/JAC_PoC_BanseikiRuleset/UI/JAC_EraCompletePopup.lua が公式の EraCompletePopup を読んだ後に、ワイルドカード
-- include("JAC_EraCompletePopup_", true) で読む。公式は OnCheckGameEraChanged がグローバルの StartEraShow を呼んで画面を出す
-- （DLC/Expansion2/UI/Additions/EraCompletePopup.lua:64,68-113）ので、それを空にする。10番目以降の時代には公式の絵が無く、
-- 出そうとすると DataError になる（同 :75-78）。
function StartEraShow() end
