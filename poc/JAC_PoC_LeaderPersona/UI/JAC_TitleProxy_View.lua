-- UI transports native operations; only saved GameCore state grants a retry.
-- Losing an in-memory callback never releases an authorized native stage.
ContextPtr:SetHide(false);
include('JAC_TitlePaymentRules');
local requestSerial = 0;
local dispatchSerial = 0;
local pendingGrant, frameAction;
local Update;
local function Active()
    local player = Players[Game.GetLocalPlayer()];
    return player and player:IsHuman() and player:IsTurnActive();
end;
local function Script(name, params)
    params = params or {};
    params.OnStart = name;
    UI.RequestPlayerOperation(Game.GetLocalPlayer(), PlayerOperations.EXECUTE_SCRIPT, params);
end;
local function NativeStage(owner, stage)
    local governors = Players[owner]:GetGovernors();
    local definition = GameInfo.Governors['GOVERNOR_JAC_TITLE_PROXY'];
    local native = governors:HasGovernor(definition.Hash) and governors:GetGovernor(definition.Hash);
    local promotion = stage > 0 and GameInfo.GovernorPromotions['GOVERNOR_PROMOTION_JAC_TITLE_PROXY_' .. stage];
    local observed = stage == 0 and native or (native and promotion and native:HasPromotion(promotion.Hash));
    return governors, definition, native, promotion, observed;
end;
local function AdvanceObserved(owner, stage)
    -- Even at zero balance, a receipt goes straight to GameCore for allocation.
    if Players[owner]:GetProperty('JAC_PROXY_NATIVE_PAID_' .. stage) ~= 1 then return false; end;
    Script('JAC_TitleProxyAdvance');
    return true;
end;
function JAC_ProxyRequest(leader)
    if not Active() then return false; end;
    local owner = Game.GetLocalPlayer();
    requestSerial = math.max(requestSerial, Players[owner]:GetProperty('JAC_PROXY_REQUEST_SERIAL') or 0) + 1;
    Script('JAC_TitleProxyRequest', {Leader=leader, RequestSerial=requestSerial});
    return requestSerial;
end;
function JAC_ProxyBeginNext()
    if not Active() then return false; end;
    Script('JAC_TitleProxyBegin');
    return true;
end;
function JAC_ProxyAdvance()
    if not Active() then return false; end;
    Script('JAC_TitleProxyAdvance');
    return true;
end;
function JAC_ProxyPayNext()
    if not Active() or pendingGrant then return false; end;
    local owner = Game.GetLocalPlayer();
    local queue = Game:GetProperty('JAC_PROXY_PAYMENT_QUEUE_' .. owner);
    local flight = queue and queue.flight;
    if not flight then return false; end;
    if AdvanceObserved(owner, flight.stage) then return true; end;
    -- Legacy flights are normalized by GameCore first, never treated as unsent.
    if not flight.state then Script('JAC_TitleProxyReconcile'); return false; end;
    local previous = Players[owner]:GetProperty('JAC_PROXY_AUTHORIZE_RESULT');
    dispatchSerial = math.max(dispatchSerial, previous and previous.serial or 0) + 1;
    pendingGrant = {owner=owner, flight=flight.id, request=flight.request,
        stage=flight.stage, attempt=flight.attempt, serial=dispatchSerial};
    Script('JAC_TitleProxyAuthorize', {Flight=flight.id, Request=flight.request,
        Stage=flight.stage, Attempt=flight.attempt, DispatchSerial=dispatchSerial});
    ContextPtr:SetUpdate(Update);
    return true;
end;
local function DispatchGrant()
    local pending = pendingGrant;
    if not pending then return; end;
    if not Active() or Game.GetLocalPlayer() ~= pending.owner then pendingGrant = nil; return; end;
    local owner = pending.owner;
    local reply = Players[owner]:GetProperty('JAC_PROXY_AUTHORIZE_RESULT');
    if not reply or reply.serial ~= pending.serial or reply.flight ~= pending.flight or reply.request ~= pending.request or
        reply.stage ~= pending.stage or reply.attempt ~= pending.attempt then return; end;
    -- This is a transport callback, not retry permission. A reloaded UI must
    -- ask GameCore again and cannot replay a saved authorization response.
    pendingGrant = nil;
    if reply.result ~= 'AUTHORIZED' then
        if reply.result == 'RECEIPT_READY' then AdvanceObserved(owner, pending.stage); end;
        return;
    end;
    local queue = Game:GetProperty('JAC_PROXY_PAYMENT_QUEUE_' .. owner);
    local flight = queue and queue.flight;
    if not flight or flight.id ~= pending.flight or flight.request ~= pending.request or
        flight.stage ~= pending.stage or flight.state ~= 'AUTHORIZED' or
        flight.attempt ~= pending.attempt + 1 or flight.lastAttemptTurn ~= Game.GetCurrentGameTurn() then return; end;
    if AdvanceObserved(owner, flight.stage) then return; end;
    local request;
    for _, item in ipairs(queue.requests) do if item.id == flight.request then request = item; break; end; end;
    if not request or request.status ~= 'PENDING' or not JAC_TitlePaymentRules.Valid(owner, request.leader) then
        Script('JAC_TitleProxyReconcile');
        return;
    end;
    local governors, definition, native, promotion, observed = NativeStage(owner, flight.stage);
    -- Acquisition can precede publication of the receipt. Wait without buying
    -- another stage; only the native property plus official spent can settle it.
    if observed then return; end;
    -- Recheck the same read-only reservation rule after grant publication.
    -- Official spending may have changed while the response was in transit.
    if not JAC_TitlePaymentRules.Next(owner) then return; end;
    if flight.stage == 0 then
        if not governors:CanAppointGovernor(definition.Hash) then return; end;
        UI.RequestPlayerOperation(owner, PlayerOperations.APPOINT_GOVERNOR,
            {[PlayerOperations.PARAM_GOVERNOR_TYPE]=definition.Index});
    else
        if not native or not promotion or not governors:CanPromoteGovernor(definition.Hash) then return; end;
        UI.RequestPlayerOperation(owner, PlayerOperations.PROMOTE_GOVERNOR,
            {[PlayerOperations.PARAM_GOVERNOR_TYPE]=definition.Index,
             [PlayerOperations.PARAM_GOVERNOR_PROMOTION_TYPE]=promotion.Index});
    end;
end;
Update = function()
    local action = frameAction;
    frameAction = nil;
    if action then action(); end;
    DispatchGrant();
    if not frameAction and not pendingGrant then ContextPtr:ClearUpdate(); end;
end;
local function OnFrame(action)
    if not Active() or frameAction or pendingGrant then return false; end;
    frameAction = action;
    ContextPtr:SetUpdate(Update);
    return true;
end;
function JAC_ProxyPrepareFixture()
    return OnFrame(function() if Active() then Script('JAC_TitleProxyFixture'); end; end);
end;
-- FireTuner only schedules these diagnostics; the common entry points and the
-- native dispatch run on real UI frames. Existing direct diagnostics still work.
function JAC_ProxyOnFrame(action, leader)
    local actions = {REQUEST=function() JAC_ProxyRequest(leader); end,
        BEGIN=JAC_ProxyBeginNext, PAY=JAC_ProxyPayNext, ADVANCE=JAC_ProxyAdvance};
    if not actions[action] then return false; end;
    return OnFrame(actions[action]);
end;
-- One payment at a time permits saving between native consumption/allocation.
print('JAC_TitleProxy_View: ready');
