PRODUCTION_ADMISSION_CASES = {}
local function gate(owner, index)
    return Players[owner]:GetProperty(901000 + index)
end
local function manifest(owner, index)
    return Players[owner]:GetProperty(900000 + index)
end
local function expect(expected, actual, label)
    assert(expected == actual, label .. ': expected ' .. tostring(expected) .. ', got ' .. tostring(actual))
end
function PRODUCTION_ADMISSION_CASES.gameplay_without_can_ever_binding_still_publishes_authoritative_owner()
    local original = Players[1].GetGovernors
    Players[1].GetGovernors = function()
        local governors = original()
        governors.CanEverAppointGovernor = nil
        return governors
    end
    assert(loadstring(GAMEPLAY_SOURCE))()
    FIRE('JAC_LeaderPersonaJoin', 1, {Source=2})
    Players[1].points = 1
    FIRE('JAC_LeaderPersonaEvolve', 1, {Leader=ASHINA})
    assert(Players[1]:GetProperty(902001)==true, 'owner-specific reset must be read')
    assert(gate(1,4)==true, 'owner 1 must publish after official reservation')
    assert(gate(0,4)==false, 'other owner remains closed')
    Players[1].GetGovernors = original
end
function PRODUCTION_ADMISSION_CASES.ai_owner_and_sparse_gameinfo_indices()
    Players[1].human = false
    ExposedMembers.JAC_LeaderPersona.Reconcile()
    for owner=0,2 do
        for row in GameInfo.JAC_LeaderPersonas() do
            local index = GameInfo.Governors[row.GovernorType].Index
            expect(true, manifest(owner, index), 'GameInfo manifest')
            expect(false, gate(owner, index), 'all unjoined people closed')
        end
    end
    FIRE('JAC_LeaderPersonaJoin', 1, {Source=2})
    expect('JOINED', Players[1]:GetProperty('JAC_PERSONA_RESULT'), 'AI Join')
    Players[1].points = 1
    FIRE('JAC_LeaderPersonaEvolve', 1, {Leader=ASHINA})
    expect('EVOLVING', Players[1]:GetProperty('JAC_PERSONA_RESULT'), 'AI reservation')
    expect(true, gate(1, 4), 'AI joined Ashina opens')
    expect(false, gate(0, 4), 'other owner remains closed')
    expect(false, gate(1, 22), 'other person remains closed')
    local id = Game:GetProperty('JAC_PERSONA_' .. ASHINA .. '_UNIT')
    local person = assert(Players[1]:GetUnits():FindID(id))
    NATIVE_UNITS[1][id] = nil
    ExposedMembers.JAC_LeaderPersona.Reconcile()
    expect(false, gate(1, 4), 'lost real person revokes')
    NATIVE_UNITS[1][id] = person
    ExposedMembers.JAC_LeaderPersona.Reconcile()
    expect(true, gate(1, 4), 'restored person requires fresh proof')
end
function PRODUCTION_ADMISSION_CASES.cost_two_reserves_all_titles_and_rejects_wrong_owner()
    ExposedMembers.JAC_LeaderPersona.Reconcile()
    FIRE('JAC_LeaderPersonaJoin', 2, {Source=1})
    expect('JOINED', Players[2]:GetProperty('JAC_PERSONA_RESULT'), 'Date Join')
    Players[2].points = 1
    FIRE('JAC_LeaderPersonaEvolve', 2, {Leader='LEADER_JAC_DATE_TANEMUNE'})
    expect('NO_TITLE', Players[2]:GetProperty('JAC_PERSONA_RESULT'), 'one title insufficient')
    expect(false, gate(2, 22), 'unpaid Date remains closed')
    Players[2].points = 2
    FIRE('JAC_LeaderPersonaEvolve', 2, {Leader='LEADER_JAC_DATE_TANEMUNE'})
    expect('EVOLVING', Players[2]:GetProperty('JAC_PERSONA_RESULT'), 'two title reservation')
    expect(true, gate(2, 22), 'actual GameInfo Index opens')
    expect(false, gate(1, 22), 'wrong owner remains closed')
    Players[2].spent = 1
    ExposedMembers.JAC_LeaderPersona.Reconcile()
    expect(false, gate(2, 22), 'official remaining titles revoke')
end

function PRODUCTION_ADMISSION_CASES.official_payments_and_core_only_final_state()
    local leader = 'LEADER_JAC_DATE_TANEMUNE'
    FIRE('JAC_LeaderPersonaJoin', 2, {Source=1})
    Players[2].points = 2
    FIRE('JAC_LeaderPersonaEvolve', 2, {Leader=leader})
    expect(true, gate(2, 22), 'full pre-appointment reservation')
    UI.RequestPlayerOperation(2, PlayerOperations.APPOINT_GOVERNOR,
        {[PlayerOperations.PARAM_GOVERNOR_TYPE]=22})
    DRAIN()
    ExposedMembers.JAC_LeaderPersona.Reconcile()
    expect(true, gate(2, 22), 'native appointment leaves one affordable title reserved')
    expect('EVOLVING',Game:GetProperty('JAC_PERSONA_'..leader..'_STATE'),
        'partial payment is not finalization or paid assignment')
    local row = GameInfo.JAC_LeaderPersonas[leader]
    local payment = GameInfo.GovernorPromotions[row.BasePromotionType .. '_TITLE_1']
    UI.RequestPlayerOperation(2, PlayerOperations.PROMOTE_GOVERNOR,
        {[PlayerOperations.PARAM_GOVERNOR_TYPE]=22,
         [PlayerOperations.PARAM_GOVERNOR_PROMOTION_TYPE]=payment.Index})
    DRAIN()
    FIRE('JAC_LeaderPersonaFinalize', 2, {Leader=leader})
    expect('EVOLVED', Players[2]:GetProperty('JAC_PERSONA_RESULT'), 'officially paid Finalize')
    expect(2, Players[2].spent, 'both official titles spent')
    expect(true, gate(2, 22), 'Core Governor and promotions verified')
    local core = Players[2].GetGovernors
    Players[2].GetGovernors = function()
        local governors = core()
        governors.GetGovernor = nil
        return governors
    end
    ExposedMembers.JAC_LeaderPersona.Reconcile()
    expect(false, gate(2, 22), 'UI state cannot replace missing Core getter')
    Players[2].GetGovernors = core
end

function PRODUCTION_ADMISSION_CASES.cost_three_needs_two_ordered_official_promotions()
    local row = GameInfo.JAC_LeaderPersonas['LEADER_JAC_ODA_NOBUHIDE']
    row.EvolutionCost = 3
    FIRE('JAC_LeaderPersonaJoin', 1, {Source=0})
    Players[1].points = 3
    FIRE('JAC_LeaderPersonaEvolve', 1, {Leader=row.LeaderType})
    expect('EVOLVING', Players[1]:GetProperty('JAC_PERSONA_RESULT'), 'cost-three reservation')
    expect(true, gate(1, 21), 'cost-three opens after full reservation')
    UI.RequestPlayerOperation(1, PlayerOperations.APPOINT_GOVERNOR,
        {[PlayerOperations.PARAM_GOVERNOR_TYPE]=21})
    DRAIN()
    for step=1,2 do
        local payment = GameInfo.GovernorPromotions[row.BasePromotionType .. '_TITLE_' .. step]
        UI.RequestPlayerOperation(1, PlayerOperations.PROMOTE_GOVERNOR,
            {[PlayerOperations.PARAM_GOVERNOR_TYPE]=21,
             [PlayerOperations.PARAM_GOVERNOR_PROMOTION_TYPE]=payment.Index})
        DRAIN()
        expect(step+1, Players[1].spent, 'official promotion spent')
    end
    FIRE('JAC_LeaderPersonaFinalize', 1, {Leader=row.LeaderType})
    expect('EVOLVED', Players[1]:GetProperty('JAC_PERSONA_RESULT'), 'cost-three Finalize')
    expect(true, gate(1, 21), 'Core confirms both ordered promotions')
end

function PRODUCTION_ADMISSION_CASES.partial_native_payment_reserves_only_each_persons_unpaid_titles()
    local leader='LEADER_JAC_ODA_NOBUHIDE'
    GameInfo.JAC_LeaderPersonas[leader].EvolutionCost=3
    FIRE('JAC_LeaderPersonaJoin',1,{Source=0})
    FIRE('JAC_LeaderPersonaJoin',1,{Source=2})
    Players[1].points=4
    FIRE('JAC_LeaderPersonaEvolve',1,{Leader=leader})
    FIRE('JAC_LeaderPersonaEvolve',1,{Leader=ASHINA})
    UI.RequestPlayerOperation(1,PlayerOperations.APPOINT_GOVERNOR,
        {[PlayerOperations.PARAM_GOVERNOR_TYPE]=21})
    DRAIN()
    local row=GameInfo.JAC_LeaderPersonas[leader]
    local first=GameInfo.GovernorPromotions[row.BasePromotionType..'_TITLE_1']
    UI.RequestPlayerOperation(1,PlayerOperations.PROMOTE_GOVERNOR,
        {[PlayerOperations.PARAM_GOVERNOR_TYPE]=21,
         [PlayerOperations.PARAM_GOVERNOR_PROMOTION_TYPE]=first.Index})
    DRAIN()
    expect(2,Players[1].spent,'appointment and first native promotion paid')
    ExposedMembers.JAC_LeaderPersona.Reconcile()
    expect(true,gate(1,4),'two remaining titles fund both remaining reservations')
    expect(true,gate(1,21),'partially paid person keeps its remaining reservation')
    Players[1].spent=1
    ExposedMembers.JAC_LeaderPersona.Reconcile()
    expect(false,gate(1,4),'native person payments cannot exceed official spending')
    Players[1].spent=3
    ExposedMembers.JAC_LeaderPersona.Reconcile()
    expect(false,gate(1,4),'unattributed spend cannot replace the other persons payment')
    Players[1].spent=2
    local core=Players[1].GetGovernors
    local ui=ExposedMembers.JAC_LeaderPersonaNativeRead
    local observed={[leader]=ui(1,leader),[ASHINA]=ui(1,ASHINA)}
    ExposedMembers.JAC_LeaderPersonaNativeRead=function(owner,person)
        if owner==1 and observed[person] then return observed[person] end
        return ui(owner,person)
    end
    Players[1].GetGovernors=function()
        local g=core(); g.GetGovernor=nil; return g
    end
    ExposedMembers.JAC_LeaderPersona.Reconcile()
    expect(false,gate(1,4),'missing Core identity cannot credit partial payment')
    Players[1].GetGovernors=core
    ExposedMembers.JAC_LeaderPersonaNativeRead=ui
    ExposedMembers.JAC_LeaderPersona.Reconcile()
    expect(true,gate(1,4),'fresh native proof restores the remaining reservation')
    local governor=core():GetGovernor(GameInfo.Governors[row.GovernorType].Hash)
    local promotionRead=governor.HasPromotion
    local second=GameInfo.GovernorPromotions[row.BasePromotionType..'_TITLE_2']
    governor.HasPromotion=function(self,hash)
        if hash==first.Hash then return false end
        if hash==second.Hash then return true end
        return promotionRead(self,hash)
    end
    ExposedMembers.JAC_LeaderPersona.Reconcile()
    expect(false,gate(1,4),'out-of-order native payments cannot reduce a reservation')
    governor.HasPromotion=promotionRead
    assert(loadstring(GAMEPLAY_SOURCE))()
    ExposedMembers.JAC_LeaderPersona.Reconcile()
    expect(true,gate(1,4),'reload recomputes payments for both people from native state')
end

function PRODUCTION_ADMISSION_CASES.unchanged_turns_do_not_republish_numeric_keys()
    ExposedMembers.JAC_LeaderPersona.Reconcile()
    local reads, writes = 0, 0
    for owner=0,2 do
        local p = Players[owner]
        local get, set = p.GetProperty, p.SetProperty
        p.GetProperty = function(self, key)
            if type(key) == 'number' then reads = reads + 1 end
            return get(self, key)
        end
        p.SetProperty = function(self, key, value)
            if type(key) == 'number' then writes = writes + 1 end
            return set(self, key, value)
        end
    end
    for turn=1,300 do
        TURN = turn
        ExposedMembers.JAC_LeaderPersona.Reconcile()
    end
    expect(0, writes, '300 unchanged turns publish no numeric deltas')
    expect(0, reads, '300 unchanged turns read no numeric admission keys')
end

function PRODUCTION_ADMISSION_CASES.stale_true_during_closure_aborts_owner_publication()
    FIRE('JAC_LeaderPersonaJoin', 1, {Source=2})
    Players[1].points = 1
    FIRE('JAC_LeaderPersonaEvolve', 1, {Leader=ASHINA})
    expect(true, gate(1, 4), 'opened before injected delayed false')
    local p, resetReads = Players[1], 0
    local set, get = p.SetProperty, p.GetProperty
    p.SetProperty = function(self, key, value)
        if key == 901004 and value == false then return end
        return set(self, key, value)
    end
    p.GetProperty = function(self, key)
        if key == 902000 + self:GetID() then resetReads = resetReads + 1 end
        return get(self, key)
    end
    p.points = 0
    ExposedMembers.JAC_LeaderPersona.Reconcile()
    assert(resetReads > 0, 'stale true must cause a naturally read owner reset')
end

function PRODUCTION_ADMISSION_CASES.owner_reuse_and_reload_reclose_saved_true()
    FIRE('JAC_LeaderPersonaJoin', 1, {Source=2})
    Players[1].points = 1
    FIRE('JAC_LeaderPersonaEvolve', 1, {Leader=ASHINA})
    expect(true, gate(1,4), 'current owner grant')
    Players[1].points = 0
    assert(loadstring(GAMEPLAY_SOURCE))()
    expect(false, gate(1,4), 'reload must close saved true before any grant')
    ExposedMembers.JAC_LeaderPersona.Reconcile()
    expect(false, gate(1,4), 'insufficient balance remains closed')
    Players[1].points = 1
    ExposedMembers.JAC_LeaderPersona.Reconcile()
    expect(true, gate(1,4), 'fresh proof after reload')
    local replacement = {}
    for k,v in pairs(Players[1]) do replacement[k]=v end
    replacement.GetProperty,replacement.SetProperty=store()
    Players[1]=replacement
    ExposedMembers.JAC_LeaderPersona.Reconcile()
    expect(true, manifest(1,4), 'new owner object gets a complete manifest')
end
function PRODUCTION_ADMISSION_CASES.missing_core_has_governor_stays_closed_and_diagnostic_reports_it()
    local leader = 'LEADER_JAC_DATE_TANEMUNE'
    FIRE('JAC_LeaderPersonaJoin', 2, {Source=1})
    Players[2].points = 2
    FIRE('JAC_LeaderPersonaEvolve', 2, {Leader=leader})
    local p, core = Players[2], Players[2].GetGovernors
    local gets = 0
    p.GetGovernors = function()
        local governors = core()
        governors.HasGovernor = nil -- actually observed missing in Gameplay
        local original = governors.GetGovernor
        governors.GetGovernor = function(self, hash)
            gets = gets + 1
            return original(self, hash)
        end
        return governors
    end
    Game:SetProperty('JAC_PERSONA_' .. leader .. '_STATE', 'GOVERNOR')
    Game:SetProperty('JAC_PERSONA_' .. leader .. '_UNIT', nil)
    NATIVE_UNITS[2] = {}
    ExposedMembers.JAC_LeaderPersona.Reconcile()
    expect(false, gate(2,22), 'missing HasGovernor must never borrow UI proof')
    gets = 0
    local result = ExposedMembers.JAC_LeaderPersona.CoreAdmissionCapabilities(2, leader)
    expect(false, result.hasGovernorBinding, 'diagnostic measures absent binding')
    expect(true, result.getGovernorBinding, 'diagnostic distinguishes available GetGovernor')
    expect(true, result.getGovernorOK, 'natural bound getter was called')
    expect(false, result.governorReturned, 'fixture has no actual governor')
    expect(1, gets, 'diagnostic invokes natural GetGovernor exactly once')
end

function PRODUCTION_ADMISSION_CASES.failed_reset_and_false_ack_never_publish_manifest()
    FIRE('JAC_LeaderPersonaJoin', 1, {Source=2})
    Players[1].points = 1
    FIRE('JAC_LeaderPersonaEvolve', 1, {Leader=ASHINA})
    local p, manifestWrites = Players[1], 0
    local get, set = p.GetProperty, p.SetProperty
    p.GetProperty = function(self, key)
        if key == 901004 then return true end -- old true after attempted false
        if key == 902000 + self:GetID() then return 'wrong-type' end
        return get(self, key)
    end
    p.SetProperty = function(self, key, value)
        if type(key) == "number" and key >= 900000 and key < 900256 then manifestWrites = manifestWrites + 1 end
        return set(self, key, value)
    end
    p.points = 0
    ExposedMembers.JAC_LeaderPersona.Reconcile()
    expect(0, manifestWrites, 'failed false ACK must abort before manifest publication')
    ExposedMembers.JAC_LeaderPersona.Reconcile()
    expect(0, manifestWrites, 'failed reset ACK must abort next initialization too')
end

function PRODUCTION_ADMISSION_CASES.owner_operation_publishes_only_changed_cells()
    FIRE('JAC_LeaderPersonaJoin', 1, {Source=2})
    local writes, byOwner = 0, {}
    for owner=0,2 do
        local p, id = Players[owner], owner
        local set = p.SetProperty
        p.SetProperty = function(self,key,value)
            if type(key) == 'number' then
                writes = writes + 1
                byOwner[id] = (byOwner[id] or 0) + 1
            end
            return set(self,key,value)
        end
    end
    Players[1].points = 1
    FIRE('JAC_LeaderPersonaEvolve', 1, {Leader=ASHINA})
    expect(4, writes, 'only close/false/manifest/true for the changed person')
    expect(4, byOwner[1], 'only authoritative owner publishes')
    expect(nil, byOwner[0], 'other owner untouched')
    expect(nil, byOwner[2], 'source owner untouched')
end

function PRODUCTION_ADMISSION_CASES.twenty_two_owners_initialize_once_and_keep_300_turns_bounded()
    for id=3,21 do
        local p={points=0,spent=0}
        p.GetID=function() return id end
        p.IsAlive=function() return true end
        p.IsMajor=function() return true end
        p.GetProperty,p.SetProperty=store()
        p.GetGovernors=function() return {
            CanEverAppointGovernor=function() return false end,
            GetGovernorPoints=function() return 0 end,
            GetGovernorPointsSpent=function() return 0 end
        } end
        Players[id]=p
    end
    PlayerManager.GetAliveMajorIDs=function()
        local ids={}
        for id=0,21 do ids[#ids+1]=id end
        return ids
    end
    local writes, reads, anchors = 0,0,0
    for id=0,21 do
        local p=Players[id]
        local get,set,governors = p.GetProperty,p.SetProperty,p.GetGovernors
        p.GetProperty=function(self,key)
            if type(key)=='number' then reads=reads+1 end
            return get(self,key)
        end
        p.SetProperty=function(self,key,value)
            if type(key)=='number' then writes=writes+1 end
            return set(self,key,value)
        end
        p.GetGovernors=function()
            local g=governors()
            local original=g.CanEverAppointGovernor
            g.CanEverAppointGovernor=function(self,hash)
                anchors=anchors+1
                return original(self,hash)
            end
            return g
        end
    end
    assert(loadstring(GAMEPLAY_SOURCE))()
    expect(22*(1+22*3),writes,'one reset and three phases per owner/index on reload')
    expect(writes,reads,'every initialization write has one natural ACK')
    expect(0,anchors,'owner reset removes the Gameplay CanEver dependency')
    for id=0,21 do
        for row in GameInfo.JAC_LeaderPersonas() do
            expect(false,gate(id,GameInfo.Governors[row.GovernorType].Index),'unjoined matrix closed')
        end
    end
    writes,reads,anchors=0,0,0
    for turn=1,300 do
        TURN=turn
        ExposedMembers.JAC_LeaderPersona.Reconcile()
        for id=0,21 do ExposedMembers.JAC_LeaderPersona.Reconcile(id) end
    end
    expect(0,writes,'whole and all player turns have no unchanged publication')
    expect(0,reads,'whole and all player turns have no admission getter traffic')
    expect(0,anchors,'Core binding anchor is not repeated per turn')
end
function PRODUCTION_ADMISSION_CASES.reload_reset_failure_still_closes_every_owner_before_aborting()
    FIRE('JAC_LeaderPersonaJoin', 1, {Source=2})
    Players[1].points=1
    FIRE('JAC_LeaderPersonaEvolve', 1, {Leader=ASHINA})
    local resetReads={}
    for id=0,2 do
        local p,owner=Players[id],id
        local get=p.GetProperty
        p.GetProperty=function(self,key)
            if key==902000+owner then
                resetReads[owner]=(resetReads[owner] or 0)+1
                if owner==0 then return 'invalid-reset' end
            end
            return get(self,key)
        end
    end
    assert(loadstring(GAMEPLAY_SOURCE))()
    for owner=0,2 do
        assert((resetReads[owner] or 0)>0,'failed first reset left another owner open: '..owner)
    end
end

function PRODUCTION_ADMISSION_CASES.sparse_alive_ids_on_reload_close_omitted_owner()
    FIRE('JAC_LeaderPersonaJoin', 1, {Source=2})
    Players[1].points=1
    FIRE('JAC_LeaderPersonaEvolve', 1, {Leader=ASHINA})
    expect(true,gate(1,4),'current permission before reload')
    local reads, original=0, Players[1].GetProperty
    Players[1].GetProperty=function(self,key)
        if key==902001 then reads=reads+1 end
        return original(self,key)
    end
    PlayerManager.GetAliveMajorIDs=function() return {[1]=0,[3]=1} end
    assert(loadstring(GAMEPLAY_SOURCE))()
    assert(reads>0,'sparse owner list must read every old owner reset')
end

function PRODUCTION_ADMISSION_CASES.legacy_schema_does_not_overwrite_a_foreign_close_key()
    for owner=0,2 do
        if owner > 0 then Players[owner]:SetProperty(902000+owner,nil) end
        for row in GameInfo.JAC_LeaderPersonas() do
            Players[owner]:SetProperty(903000+GameInfo.Governors[row.GovernorType].Index,nil)
        end
    end
    Game:SetProperty('JAC_PERSONA_ADMISSION_SCHEMA_V1',1)
    Players[1]:SetProperty(903004,'foreign-owner-data')
    assert(loadstring(GAMEPLAY_SOURCE))()
    expect('foreign-owner-data',Players[1]:GetProperty(903004),'legacy schema never owned new close range')
    expect(1,Game:GetProperty('JAC_PERSONA_ADMISSION_SCHEMA_V1'),'collision aborts migration')
end

function PRODUCTION_ADMISSION_CASES.legacy_schema_claims_empty_close_range_before_republishing()
    for owner=0,2 do
        if owner > 0 then Players[owner]:SetProperty(902000+owner,nil) end
        for row in GameInfo.JAC_LeaderPersonas() do
            Players[owner]:SetProperty(903000+GameInfo.Governors[row.GovernorType].Index,nil)
        end
    end
    Game:SetProperty('JAC_PERSONA_ADMISSION_SCHEMA_V1',1)
    assert(loadstring(GAMEPLAY_SOURCE))()
    expect(3,Game:GetProperty('JAC_PERSONA_ADMISSION_SCHEMA_V1'),'empty legacy range safely migrates')
    for owner=0,2 do
        expect(false,gate(owner,4),'migration closes any earlier permission')
        expect(true,manifest(owner,4),'current manifest after migration')
    end
end

function PRODUCTION_ADMISSION_CASES.schema_two_rejects_foreign_owner_reset_key()
    for owner=1,2 do Players[owner]:SetProperty(902000+owner,nil) end
    Game:SetProperty('JAC_PERSONA_ADMISSION_SCHEMA_V1',2)
    Players[1]:SetProperty(902001,'foreign-reset')
    assert(loadstring(GAMEPLAY_SOURCE))()
    expect(2,Game:GetProperty('JAC_PERSONA_ADMISSION_SCHEMA_V1'),'collision keeps old schema')
    expect('foreign-reset',Players[1]:GetProperty(902001),'foreign reset remains untouched')
end

function PRODUCTION_ADMISSION_CASES.dead_major_foreign_keys_are_not_claimed_on_revival()
    local schema = 'JAC_PERSONA_ADMISSION_SCHEMA_V1'
    local original = PlayerManager.GetAliveMajorIDs
    Players[2].alive = false
    PlayerManager.GetAliveMajorIDs = function() return {0,1} end
    for _,foreignKey in ipairs({900004,901004,902002,903004}) do
        Game:SetProperty(schema,nil)
        for owner=0,2 do
            local p = Players[owner]
            p:SetProperty(902000+owner,nil)
            for row in GameInfo.JAC_LeaderPersonas() do
                local index = GameInfo.Governors[row.GovernorType].Index
                for _,base in ipairs({900000,901000,903000}) do
                    p:SetProperty(base+index,nil)
                end
            end
        end
        Players[2]:SetProperty(foreignKey,'foreign-dead-major-value')
        assert(loadstring(GAMEPLAY_SOURCE))()
        expect(nil,Game:GetProperty(schema),'foreign value on a dead major blocks initial ownership')
        expect('foreign-dead-major-value',Players[2]:GetProperty(foreignKey),'foreign value is preserved')
        Players[2].alive = true
        PlayerManager.GetAliveMajorIDs = function() return {0,1,2} end
        ExposedMembers.JAC_LeaderPersona.Reconcile()
        expect('foreign-dead-major-value',Players[2]:GetProperty(foreignKey),'revival must not overwrite foreign data')
        Players[2].alive = false
        PlayerManager.GetAliveMajorIDs = function() return {0,1} end
    end
    Players[2].alive = true
    PlayerManager.GetAliveMajorIDs = original
end

function PRODUCTION_ADMISSION_CASES.dead_major_getter_failure_closes_every_owner()
    local schema = 'JAC_PERSONA_ADMISSION_SCHEMA_V1'
    Game:SetProperty(schema,nil)
    for owner=0,2 do
        local p = Players[owner]
        p:SetProperty(902000+owner,nil)
        for row in GameInfo.JAC_LeaderPersonas() do
            local index = GameInfo.Governors[row.GovernorType].Index
            for _,base in ipairs({900000,901000,903000}) do
                p:SetProperty(base+index,nil)
            end
        end
    end
    Players[2].alive = false
    PlayerManager.GetAliveMajorIDs = function() return {0,1} end
    local resetReads = {}
    for owner=0,2 do
        local p, id = Players[owner], owner
        local get = p.GetProperty
        p.GetProperty = function(self,key)
            if key == 902000+id then resetReads[id] = (resetReads[id] or 0)+1 end
            if id == 2 and key == 900004 then error('injected dead-major getter failure') end
            return get(self,key)
        end
    end
    assert(loadstring(GAMEPLAY_SOURCE))()
    expect(nil,Game:GetProperty(schema),'failed getter must not claim the namespace')
    for owner=0,2 do
        assert((resetReads[owner] or 0)>0,'failure must close owner '..owner)
    end
end

function PRODUCTION_ADMISSION_CASES.legacy_getter_failure_preserves_schema_and_closes_every_owner()
    local schema = 'JAC_PERSONA_ADMISSION_SCHEMA_V1'
    for _,case in ipairs({{version=1,failed=903004,foreign=903154},
                           {version=2,failed=902002,foreign=902002}}) do
        for owner=0,2 do
            local p = Players[owner]
            p:SetProperty(902000+owner,nil)
            for row in GameInfo.JAC_LeaderPersonas() do
                local index = GameInfo.Governors[row.GovernorType].Index
                p:SetProperty(903000+index,nil)
            end
        end
        Players[2]:SetProperty(case.foreign,'foreign-legacy-value')
        Game:SetProperty(schema,case.version)
        local resetReads, originals = {}, {}
        for owner=0,2 do
            local p, id = Players[owner], owner
            originals[id] = p.GetProperty
            p.GetProperty = function(self,key)
                if key == 902000+id then resetReads[id] = (resetReads[id] or 0)+1 end
                if id == 2 and key == case.failed then error('injected legacy getter failure') end
                return originals[id](self,key)
            end
        end
        assert(loadstring(GAMEPLAY_SOURCE))()
        expect(case.version,Game:GetProperty(schema),'getter failure keeps the previous schema')
        for owner=0,2 do
            assert((resetReads[owner] or 0)>0,'legacy failure must close owner '..owner)
            Players[owner].GetProperty = originals[owner]
        end
        expect('foreign-legacy-value',Players[2]:GetProperty(case.foreign),'foreign legacy value is preserved')
    end
end
