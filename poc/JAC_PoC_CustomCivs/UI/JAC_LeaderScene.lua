-- 2D 背景を使う両家では標準の黒い上下レターボックスを隠す。
-- ベースの LeaderScene に背景生成とイベント処理を任せる。
include("LeaderScene");

local function SetCustomCivLetterboxes(playerID)
    if playerID == nil then return end
    local config = PlayerConfigurations[playerID];
    if config == nil then return end
    local leader = config:GetLeaderTypeName();
    local custom = leader == "LEADER_JAC_ODA_NOBUHIDE" or leader == "LEADER_JAC_TAKEDA_NOBUTORA";
    Controls.TopLetterbox:SetHide(custom);
    Controls.BottomLetterbox:SetHide(custom);
end

LuaEvents.DiploScene_LeaderSelect.Add(SetCustomCivLetterboxes);
LuaEvents.DiploScene_SceneOpened.Add(SetCustomCivLetterboxes);
