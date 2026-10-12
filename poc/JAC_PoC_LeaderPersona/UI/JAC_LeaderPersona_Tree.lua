include("InstanceManager");
include("JAC_PersonaTreeRules");
local Rules = JAC_PersonaTreeRules;
local m_Categories = InstanceManager:new("PersonaCategory", "Root", Controls.Categories);
local m_Nodes = InstanceManager:new("PersonaSkill", "Root", Controls.Nodes);
local m_Leader, m_Category, m_Pending;
local m_Submitted, m_RequestSerial = nil, 0;
local m_List, m_Index = {}, 1;
local m_CategoryList = {};
local function Active()
    local player = Players[Game.GetLocalPlayer()];
    return player and player:IsTurnActive();
end
local function Effect(node)
    if node.EffectKind == "AMENITIES" then return Locale.Lookup("LOC_JAC_PERSONA_EFFECT_AMENITIES", node.Amount); end
    return Locale.Lookup("LOC_JAC_PERSONA_EFFECT", Locale.Lookup("LOC_JAC_PERSONA_EFFECT_" .. node.EffectKind), node.Amount);
end
local function Select(index, scroll)
    if #m_List == 0 then return; end
    m_Index = math.max(1, math.min(#m_List, index));
    local node = m_List[m_Index];
    Controls.Selection:SetText(Locale.Lookup("LOC_JAC_PERSONA_SELECTED", Locale.Lookup(node.Name)));
    local text = Effect(node);
    if node.PrereqNode then
        text = text .. " " .. Locale.Lookup("LOC_JAC_PERSONA_REQUIRES", Locale.Lookup(GameInfo.JAC_PersonaTreeNodes[node.PrereqNode].Name));
    end
    Controls.Effect:SetText(text);
    if scroll then Controls.NodesScroll:SetScrollValue(#m_List > 1 and (m_Index-1)/(#m_List-1) or 0); end
end
function JAC_TreeRefresh()
    if not m_Leader then return; end
    local person = GameInfo.JAC_LeaderPersonas[m_Leader];
    if not person or Rules.Get(m_Leader, "OWNER") ~= Game.GetLocalPlayer() then ContextPtr:SetHide(true); return; end
    Controls.Name:SetText(Locale.Lookup(person.Name)); Controls.Portrait:SetIcon(person.PortraitIcon, 64);
    Controls.Merit:SetText(Locale.Lookup("LOC_JAC_PERSONA_MERIT", Rules.Merit(m_Leader)));
    Controls.Merit:SetToolTipString(Locale.Lookup("LOC_JAC_PERSONA_MERIT_HELP"));
    local committed = Rules.Get(m_Leader, "CATEGORY");
    Controls.Commitment:SetText(committed and Locale.Lookup("LOC_JAC_PERSONA_CATEGORY_LOCKED",
        Locale.Lookup(GameInfo.JAC_PersonaCategories[committed].Name)) or Locale.Lookup("LOC_JAC_PERSONA_UNCOMMITTED"));
    m_CategoryList = Rules.Categories(m_Leader);
    m_Categories:ResetInstances();
    for _,category in ipairs(m_CategoryList) do
        local type = category.CategoryType;
        local view = m_Categories:GetInstance();
        local text = Locale.Lookup(category.Name);
        if committed and type ~= committed then text = text .. " ×"; end
        if type == m_Category then text = "[ " .. text .. " ]"; end
        view.Root:SetText(text);
        view.Root:SetToolTipString(Locale.Lookup(committed and type ~= committed and
            "LOC_JAC_PERSONA_NODE_STATUS_CATEGORY_LOCKED" or "LOC_JAC_PERSONA_PERMANENT"));
        view.Root:RegisterCallback(Mouse.eLClick, function() JAC_TreeCategory(type); end);
    end
    m_List = Rules.Nodes(m_Leader, m_Category); m_Nodes:ResetInstances();
    for index,node in ipairs(m_List) do
        local view = m_Nodes:GetInstance();
        local status = Rules.Status(Game.GetLocalPlayer(), m_Leader, node.NodeType);
        view.Route:SetText(Locale.Lookup(node.RouteName) .. (node.Signature == 1 and " / " .. Locale.Lookup("LOC_JAC_PERSONA_SIGNATURE") or ""));
        view.Node:SetText(Locale.Lookup("LOC_JAC_PERSONA_NODE", Locale.Lookup(node.Name),
            Locale.Lookup("LOC_JAC_PERSONA_NODE_STATUS_" .. status), node.Cost));
        view.Effect:SetText(Effect(node));
        view.Root:SetDisabled(status ~= "AVAILABLE" or not Active() or m_Pending ~= nil);
        view.Root:SetToolTipString(node.PrereqNode and Locale.Lookup("LOC_JAC_PERSONA_REQUIRES",
            Locale.Lookup(GameInfo.JAC_PersonaTreeNodes[node.PrereqNode].Name)) or Locale.Lookup("LOC_JAC_PERSONA_PERMANENT"));
        local selected, nodeType = index, node.NodeType;
        view.Root:RegisterCallback(Mouse.eLClick, function() Select(selected); JAC_TreeReview(nodeType); end);
    end
    Controls.Categories:CalculateSize(); Controls.Nodes:CalculateSize(); Controls.NodesScroll:CalculateInternalSize();
    Select(m_Index);
    if m_Pending then
        Controls.Confirm:SetDisabled(not Active() or Rules.Status(Game.GetLocalPlayer(), m_Leader, m_Pending) ~= "AVAILABLE");
    end
end
function JAC_TreeCategory(category)
    if m_Pending then return; end
    for _,row in ipairs(Rules.Categories(m_Leader)) do
        if row.CategoryType == category then m_Category, m_Index = category, 1; JAC_TreeRefresh(); Controls.NodesScroll:SetScrollValue(0); return; end
    end
end
function JAC_TreeOpen(leader, nodeType)
    if type(leader) ~= "string" then return false; end
    if Rules.Get(leader, "OWNER") ~= Game.GetLocalPlayer() or Rules.Get(leader, "STATE") ~= "GOVERNOR" then return false; end
    m_Leader, m_Pending, m_Index, m_Submitted = leader, nil, 1, nil;
    m_Category = Rules.Get(leader, "CATEGORY") or GameInfo.JAC_PersonaTreePeople[leader].PreferredCategory;
    if nodeType then
        local node = GameInfo.JAC_PersonaTreeNodes[nodeType];
        if not node or node.LeaderType ~= leader then return false; end
        m_Category = node.CategoryType;
    end
    Controls.Confirmation:SetHide(true); Controls.Result:SetText(""); ContextPtr:SetHide(false); JAC_TreeRefresh();
    Controls.NodesScroll:SetScrollValue(0);
    if nodeType then return JAC_TreeReview(nodeType); end
    return true;
end
function JAC_TreeReview(nodeType)
    if not Active() or m_Pending then return false; end
    local status = Rules.Status(Game.GetLocalPlayer(), m_Leader, nodeType);
    if status ~= "AVAILABLE" then Controls.Result:SetText(Locale.Lookup("LOC_JAC_PERSONA_NODE_STATUS_" .. status)); return false; end
    local node = GameInfo.JAC_PersonaTreeNodes[nodeType];
    m_Pending = nodeType;
    local categories, routes = Rules.Locks(m_Leader, node);
    local locks = {};
    for _,category in ipairs(categories) do table.insert(locks, Locale.Lookup(category.Name)); end
    for _,route in ipairs(routes) do table.insert(locks, Locale.Lookup(route.Name)); end
    Controls.ConfirmText:SetText(Locale.Lookup("LOC_JAC_PERSONA_CONFIRM", Locale.Lookup(node.Name), node.Cost));
    Controls.Locks:SetText(#locks > 0 and Locale.Lookup("LOC_JAC_PERSONA_LOCKS", table.concat(locks, " / ")) or Locale.Lookup("LOC_JAC_PERSONA_NO_NEW_LOCKS"));
    Controls.Confirmation:SetHide(false); JAC_TreeRefresh();
    return true;
end
function JAC_TreeCancel()
    m_Pending = nil; Controls.Confirmation:SetHide(true); JAC_TreeRefresh();
end
function JAC_TreeConfirm()
    if not m_Pending or not Active() then return false; end
    if Rules.Status(Game.GetLocalPlayer(), m_Leader, m_Pending) ~= "AVAILABLE" then JAC_TreeCancel(); return false; end
    local node = GameInfo.JAC_PersonaTreeNodes[m_Pending];
    local player = Players[Game.GetLocalPlayer()];
    m_RequestSerial = math.max(m_RequestSerial, player:GetProperty("JAC_PERSONA_PURCHASE_SERIAL") or 0) + 1;
    m_Submitted = {Leader=m_Leader, Node=m_Pending, Serial=m_RequestSerial};
    UI.RequestPlayerOperation(Game.GetLocalPlayer(), PlayerOperations.EXECUTE_SCRIPT,
        {OnStart="JAC_LeaderPersonaPurchase", Leader=m_Leader, Node=m_Pending,
         ConfirmCategory=node.CategoryType, ConfirmRoute=node.RouteType, RequestSerial=m_RequestSerial});
    JAC_TreeCancel(); return true;
end
local function NextPerson()
    if m_Pending then return; end
    local leaders = {};
    for row in GameInfo.JAC_LeaderPersonas() do
        if Rules.Get(row.LeaderType, "OWNER") == Game.GetLocalPlayer() and Rules.Get(row.LeaderType, "STATE") == "GOVERNOR" then
            table.insert(leaders, row.LeaderType);
        end
    end
    table.sort(leaders);
    for index,leader in ipairs(leaders) do if leader == m_Leader then JAC_TreeOpen(leaders[index % #leaders + 1]); return; end end
end
function JAC_TreeInput(input)
    if ContextPtr:IsHidden() then return false; end
    if input:GetMessageType() ~= KeyEvents.KeyUp then return false; end
    local key = input:GetKey();
    if key == Keys.VK_ESCAPE then
        if m_Pending then JAC_TreeCancel(); else ContextPtr:SetHide(true); end
    elseif key == Keys.VK_RETURN then
        if m_Pending then JAC_TreeConfirm(); elseif m_List[m_Index] then JAC_TreeReview(m_List[m_Index].NodeType); end
    elseif not m_Pending and key == Keys.VK_TAB then NextPerson();
    elseif not m_Pending and (key == Keys.VK_UP or key == Keys.VK_DOWN) then Select(m_Index + (key == Keys.VK_UP and -1 or 1), true);
    elseif not m_Pending and (key == Keys.VK_LEFT or key == Keys.VK_RIGHT) then
        for index,category in ipairs(m_CategoryList) do
            if category.CategoryType == m_Category then
                JAC_TreeCategory(m_CategoryList[(index-1+(key == Keys.VK_LEFT and -1 or 1)) % #m_CategoryList + 1].CategoryType); break;
            end
        end
    else return false; end
    return true;
end
Controls.Confirm:RegisterCallback(Mouse.eLClick, JAC_TreeConfirm);
Controls.Cancel:RegisterCallback(Mouse.eLClick, JAC_TreeCancel);
Controls.Close:RegisterCallback(Mouse.eLClick, function() JAC_TreeCancel(); ContextPtr:SetHide(true); end);
Controls.NextPerson:RegisterCallback(Mouse.eLClick, NextPerson);
ContextPtr:SetInputHandler(JAC_TreeInput, true);
LuaEvents.JAC_PersonaTree_Open.Add(JAC_TreeOpen);
Events.GameCoreEventPublishComplete.Add(function()
    if not ContextPtr:IsHidden() then
        local player = Players[Game.GetLocalPlayer()];
        if m_Submitted and player and
           player:GetProperty("JAC_PERSONA_PURCHASE_SERIAL") == m_Submitted.Serial and
           player:GetProperty("JAC_PERSONA_PURCHASE_LEADER") == m_Submitted.Leader and
           player:GetProperty("JAC_PERSONA_PURCHASE_NODE") == m_Submitted.Node then
            local result = player:GetProperty("JAC_PERSONA_PURCHASE_RESULT");
            if result then Controls.Result:SetText(Locale.Lookup("LOC_JAC_PERSONA_NODE_STATUS_" .. result)); end
            m_Submitted = nil;
        end
        JAC_TreeRefresh();
    end
end);
Events.LocalPlayerTurnEnd.Add(JAC_TreeCancel);
Events.LocalPlayerChanged.Add(function() JAC_TreeCancel(); ContextPtr:SetHide(true); end);
