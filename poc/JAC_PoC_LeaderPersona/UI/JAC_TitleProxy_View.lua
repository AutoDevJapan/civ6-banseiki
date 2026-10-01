-- Diagnostic UI submits the same synchronized operations as the future view.
-- Native reads may decide when to request Advance; GameCore alone proves payment.
ContextPtr:SetHide(false);
include('JAC_TitlePaymentRules');
local attempted = {};
local requestSerial = 0;
local function Active()
    local player = Players[Game.GetLocalPlayer()];
    return player and player:IsHuman() and player:IsTurnActive();
end;
function JAC_ProxyRequest(leader)
    if not Active() then return false; end;
    local owner = Game.GetLocalPlayer();
    requestSerial = math.max(requestSerial, Players[owner]:GetProperty('JAC_PROXY_REQUEST_SERIAL') or 0) + 1;
    UI.RequestPlayerOperation(Game.GetLocalPlayer(), PlayerOperations.EXECUTE_SCRIPT,
        {OnStart='JAC_TitleProxyRequest', Leader=leader, RequestSerial=requestSerial});
    return requestSerial;
end;
function JAC_ProxyPrepareFixture()
    if not Active() then return false; end;
    ContextPtr:SetUpdate(function()
        ContextPtr:ClearUpdate();
        UI.RequestPlayerOperation(Game.GetLocalPlayer(), PlayerOperations.EXECUTE_SCRIPT,
            {OnStart='JAC_TitleProxyFixture'});
    end);
    return true;
end;
function JAC_ProxyPayNext()
    if not Active() then return false; end;
    local owner = Game.GetLocalPlayer();
    local queue = Game:GetProperty('JAC_PROXY_PAYMENT_QUEUE_' .. owner);
    if not queue then return false; end;
    local request;
    for _, item in ipairs(queue.requests) do if item.status == 'PENDING' then request = item; break; end; end;
    if not request then return false; end;
    if not JAC_TitlePaymentRules.Valid(owner, request.leader) then
        UI.RequestPlayerOperation(owner, PlayerOperations.EXECUTE_SCRIPT, {OnStart='JAC_TitleProxyReconcile'});
        return false;
    end;
    local descriptor = JAC_TitlePaymentRules.Next(owner);
    if not descriptor then return false; end;
    local stage = queue.cursor;
    local governors = Players[owner]:GetGovernors();
    local definition = GameInfo.Governors['GOVERNOR_JAC_TITLE_PROXY'];
    local native = governors:HasGovernor(definition.Hash) and governors:GetGovernor(definition.Hash);
    local promotion = stage > 0 and GameInfo.GovernorPromotions['GOVERNOR_PROMOTION_JAC_TITLE_PROXY_' .. stage];
    local observed = stage == 0 and native or (native and promotion and native:HasPromotion(promotion.Hash));
    if observed then
        -- No native balance can be fabricated by this message: Advance reads
        -- the exact native modifier receipt in the gameplay context.
        if Players[owner]:GetProperty('JAC_PROXY_NATIVE_PAID_' .. stage) ~= 1 then return false; end;
        UI.RequestPlayerOperation(owner, PlayerOperations.EXECUTE_SCRIPT, {OnStart='JAC_TitleProxyAdvance'});
        return true;
    end;
    local otherReservations = 0;
    for _, item in ipairs(queue.requests) do
        if item.status == 'PENDING' and item.id ~= request.id then
            otherReservations = otherReservations + item.cost - #item.payments;
        end;
    end;
    if governors:GetGovernorPoints() - governors:GetGovernorPointsSpent() - otherReservations < 1 then return false; end;
    local key = request.id .. ':' .. stage .. ':native:' .. Game.GetCurrentGameTurn();
    if attempted[key] then return false; end;
    if stage == 0 then
        if not governors:CanAppointGovernor(definition.Hash) then return false; end;
        UI.RequestPlayerOperation(owner, PlayerOperations.APPOINT_GOVERNOR,
            {[PlayerOperations.PARAM_GOVERNOR_TYPE]=definition.Index});
    else
        if not native or not promotion or not governors:CanPromoteGovernor(definition.Hash) then return false; end;
        UI.RequestPlayerOperation(owner, PlayerOperations.PROMOTE_GOVERNOR,
            {[PlayerOperations.PARAM_GOVERNOR_TYPE]=definition.Index,
             [PlayerOperations.PARAM_GOVERNOR_PROMOTION_TYPE]=promotion.Index});
    end;
    attempted[key] = true;
    return true;
end;
-- The driver deliberately requests one payment at a time, so it can save
-- between payments rather than racing an automatic chain.
print('JAC_TitleProxy_View: ready');
