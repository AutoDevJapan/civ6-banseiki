-- Pure shared read rules; UI and Gameplay entry files have distinct names.
JAC_PersonaTreeRules = {};
local Rules = JAC_PersonaTreeRules;
function Rules.Key(leader, field) return "JAC_PERSONA_" .. leader .. "_" .. field; end
function Rules.Get(leader, field) return Game:GetProperty(Rules.Key(leader, field)); end
function Rules.Merit(leader) return Rules.Get(leader, "MERIT") or 0; end
function Rules.ReservedAppointments(owner)
    local reserved = 0;
    for person in GameInfo.JAC_LeaderPersonas() do
        if Rules.Get(person.LeaderType, "OWNER") == owner and Rules.Get(person.LeaderType, "STATE") == "EVOLVING" then
            local cost = Rules.Get(person.LeaderType, "EVOLUTION_COST") or person.EvolutionCost;
            local native = Rules.NativeGovernor(owner, person.LeaderType);
            local paid = native and native.appointed and (1 + (native.surchargePaid or 0)) or 0;
            reserved = reserved + math.max(0, cost - paid);
        end
    end
    return reserved;
end
function Rules.Owned(node) return Rules.Get(node.LeaderType, "NODE_" .. node.NodeType) == 1; end
function Rules.Nodes(leader, category)
    local nodes = {};
    for node in GameInfo.JAC_PersonaTreeNodes() do
        if node.LeaderType == leader and (not category or node.CategoryType == category) then
            table.insert(nodes, node);
        end
    end
    table.sort(nodes, function(a,b)
        if a.CategoryType ~= b.CategoryType then
            return GameInfo.JAC_PersonaCategories[a.CategoryType].SortOrder < GameInfo.JAC_PersonaCategories[b.CategoryType].SortOrder;
        end
        if a.SortOrder ~= b.SortOrder then return a.SortOrder < b.SortOrder; end
        if a.Depth ~= b.Depth then return a.Depth < b.Depth; end
        return a.NodeType < b.NodeType;
    end);
    return nodes;
end
function Rules.Categories(leader)
    local result = {};
    for access in GameInfo.JAC_PersonaCategoryAccess() do
        if access.LeaderType == leader then table.insert(result, GameInfo.JAC_PersonaCategories[access.CategoryType]); end
    end
    table.sort(result, function(a,b) return a.SortOrder < b.SortOrder; end);
    return result;
end
-- UI supplies a fresh read of engine state, never a cached property or operation parameter.
-- Absence during UI initialization is unavailable, not evidence of a missing governor.
function Rules.NativeGovernor(owner, leader)
    local read = ExposedMembers.JAC_LeaderPersonaNativeRead;
    if type(read) ~= "function" then return nil; end
    return read(owner, leader);
end
function Rules.Status(owner, leader, nodeType)
    if type(leader) ~= "string" then return "NOT_OWNED"; end
    local person = GameInfo.JAC_LeaderPersonas[leader];
    if not person or Rules.Get(leader, "OWNER") ~= owner then return "NOT_OWNED"; end
    local node = type(nodeType) == "string" and GameInfo.JAC_PersonaTreeNodes[nodeType];
    if not node or node.LeaderType ~= leader then return "INVALID_NODE"; end
    -- Gameplay commits GOVERNOR only after it has verified native appointment and
    -- the base promotion. A purchase must not depend on a local UI reader later.
    if not Players[owner] or Rules.Get(leader, "STATE") ~= "GOVERNOR" then return "NOT_GOVERNOR"; end
    if Rules.Owned(node) then return "OWNED"; end
    local category = Rules.Get(leader, "CATEGORY");
    if category and category ~= node.CategoryType then return "CATEGORY_LOCKED"; end
    if node.ExclusiveGroup then
        local route = Rules.Get(leader, "BRANCH_" .. node.CategoryType .. "_" .. node.ExclusiveGroup);
        if route and route ~= node.RouteType then return "BRANCH_LOCKED"; end
    end
    if node.PrereqNode and Rules.Get(leader, "NODE_" .. node.PrereqNode) ~= 1 then return "PREREQ_LOCKED"; end
    if Rules.Merit(leader) < node.Cost then return "NO_MERIT"; end
    return "AVAILABLE";
end
-- Actual newly lost choices, shared by the confirmation screen and automated assertions.
function Rules.Locks(leader, node)
    local categories, routes = {}, {};
    if not Rules.Get(leader, "CATEGORY") then
        for _,category in ipairs(Rules.Categories(leader)) do
            if category.CategoryType ~= node.CategoryType then table.insert(categories, category); end
        end
    end
    if node.ExclusiveGroup and not Rules.Get(leader, "BRANCH_" .. node.CategoryType .. "_" .. node.ExclusiveGroup) then
        for route in GameInfo.JAC_PersonaRouteTemplates() do
            if route.CategoryType == node.CategoryType and route.ExclusiveGroup == node.ExclusiveGroup and
               route.RouteType ~= node.RouteType then table.insert(routes, route); end
        end
        table.sort(routes, function(a,b) return a.RouteType < b.RouteType; end);
    end
    return categories, routes;
end
