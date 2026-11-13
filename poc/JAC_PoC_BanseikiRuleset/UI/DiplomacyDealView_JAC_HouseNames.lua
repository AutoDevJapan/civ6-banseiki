-- The official diplomacy screen loads DiplomacyDealView_* before Initialize.
-- Install the display adapter in that screen without replacing its trade logic.
include("JAC_HouseNames")
JAC_InstallHouseNames("DiplomacyDealView")
