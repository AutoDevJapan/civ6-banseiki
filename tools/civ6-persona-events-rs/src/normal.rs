//! Normal Gameplay acquisition sets. This is the sole source of the shipped Lua.
pub const ADAPTER: &str = r#"
-- Normal receipts are independent of the private cost-two diagnostic.
do
local E=PersonaEvents
local STORE='JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1'
local SOURCE='civ6-persona-events-rs/v3/normal-Gameplay'
local registrations={}
local function subscribed() return registrations.GovernorAppointed==true and registrations.GovernorPromoted==true end
local function integer(n,limit) return type(n)=='number' and n>=0 and n<(limit or 9007199254740991) and n%1==0 end
local function hash(n) return type(n)=='number' and n>=-2147483648 and n<=4294967295 and n%1==0 end
local function copy(v)
    if type(v)~='table' then return v end
    local t={}; for k,x in pairs(v) do t[k]=copy(x) end; return t
end
local function spent(owner)
    local p=Player(owner,true); local g=p and p:GetGovernors()
    if not g or type(g.GetGovernorPointsSpent)~='function' then return nil end
    local ok,n=pcall(g.GetGovernorPointsSpent,g); if ok and integer(n) then return n end
end
local function catalog()
    local rows,leaders,governors,promotions,hashes,people,personHashes={},{},{},{},{},{},{}
    local governorIndices,governorTypes,governorHashes={},{},{}
    for g in GameInfo.Governors() do
        if not integer(g.Index,256) or not hash(g.Hash) or type(g.GovernorType)~='string' or
           governorIndices[g.Index] or governorTypes[g.GovernorType] or governorHashes[g.Hash] then return nil end
        governorIndices[g.Index]=g.GovernorType; governorTypes[g.GovernorType]=g.Index; governorHashes[g.Hash]=true
    end
    -- Validate the entire official promotion catalog, including unknown extras.
    local indices,types,promotionHashes={},{},{}
    for p in GameInfo.GovernorPromotions() do
        if not integer(p.Index,4096) or type(p.GovernorPromotionType)~='string' or
           not hash(p.Hash) or indices[p.Index] or types[p.GovernorPromotionType] or promotionHashes[p.Hash] then return nil end
        indices[p.Index]=p; types[p.GovernorPromotionType]=p
        promotionHashes[p.Hash]=true
    end
    -- Membership, including BASE, is engine data. Names never imply a Set.
    if not GameInfo.GovernorPromotionSets then return nil end
    local membership,baseByGovernor,setBinding={},{},{}
    for set in GameInfo.GovernorPromotionSets() do
        local gi=governorTypes[set.GovernorType]; local p=types[set.GovernorPromotion]
        if gi==nil or not p then return nil end
        membership[gi]=membership[gi] or {}
        if membership[gi][p.Index] then return nil end
        membership[gi][p.Index]=true
        if p.BaseAbility==true or p.BaseAbility==1 then
            if baseByGovernor[gi] then return nil end
            baseByGovernor[gi]=p.Index
        end
        local g=GameInfo.Governors[set.GovernorType]
        setBinding[#setBinding+1]=table.concat({g.GovernorType,gi,g.Hash,p.GovernorPromotionType,p.Index,p.Hash,
            (p.BaseAbility==true or p.BaseAbility==1) and 1 or 0},'/')
    end
    for row in GameInfo.JAC_LeaderPersonas() do
        local l=row.LeaderType; local g=GameInfo.Governors[row.GovernorType]
        local person=GameInfo.GreatPersonIndividuals[row.PersonType]
        if type(l)~='string' or leaders[l] or not integer(row.EvolutionCost,4) or row.EvolutionCost<1 or
           row.GovernorType~='GOVERNOR_JAC_PERSONA_'..string.sub(l,12) or
           row.BasePromotionType~='GOVERNOR_PROMOTION_JAC_PERSONA_'..string.sub(l,12)..'_BASE' or
           not g or g.GovernorType~=row.GovernorType or not integer(g.Index,256) or
           governorIndices[g.Index]~=row.GovernorType or governorTypes[row.GovernorType]~=g.Index or
           not hash(g.Hash) or governors[g.Index] or hashes[g.Hash] or
           row.PersonType~='GREAT_PERSON_INDIVIDUAL_JAC_PERSONA_'..string.sub(l,12) or
           not person or person.GreatPersonIndividualType~=row.PersonType or
           not integer(person.Index,4096) or not hash(person.Hash) or people[person.Index] or personHashes[person.Hash] or
           type(row.CivilizationType)~='string' then return nil end
        local d={leader=l,governor=g.Index,governorHash=g.Hash,governorType=g.GovernorType,
            person=person.Index,personHash=person.Hash,personType=row.PersonType,
            civilization=row.CivilizationType,baseType=row.BasePromotionType,cost=row.EvolutionCost,promotions={},byIndex={}}
        for step=0,d.cost-1 do
            local name=row.BasePromotionType..(step==0 and '' or '_TITLE_'..step)
            local p=GameInfo.GovernorPromotions[name]
            if not p or p.GovernorPromotionType~=name or not indices[p.Index] or
               indices[p.Index].GovernorPromotionType~=name or types[name].Index~=p.Index or
               not hash(p.Hash) or promotions[p.Index] or not membership[g.Index] or
               not membership[g.Index][p.Index] or (step==0 and baseByGovernor[g.Index]~=p.Index) then return nil end
            promotions[p.Index]=true
            d.promotions[step+1]={index=p.Index,hash=p.Hash,name=name}; d.byIndex[p.Index]=step+1
        end
        rows[#rows+1]=d; leaders[l]=d; governors[g.Index]=d; hashes[g.Hash]=true
        people[person.Index]=true; personHashes[person.Hash]=true
    end
    if #rows~=22 then return nil end
    table.sort(rows,function(a,b) return a.governor<b.governor end)
    local parts={SOURCE}
    table.sort(setBinding)
    for _,set in ipairs(setBinding) do parts[#parts+1]=#set..':'..set end
    for _,d in ipairs(rows) do
        for _,v in ipairs({d.leader,d.governorType,d.governor,d.governorHash,d.personType,d.person,d.personHash,d.civilization,d.cost}) do
            local s=tostring(v); parts[#parts+1]=#s..':'..s
        end
        for _,p in ipairs(d.promotions) do
            for _,v in ipairs({p.name,p.index,p.hash}) do local s=tostring(v); parts[#parts+1]=#s..':'..s end
        end
    end
    return {rows=rows,leaders=leaders,governors=governors,allGovernors=governorIndices,
        allPromotions=indices,membership=membership,baseByGovernor=baseByGovernor,binding=table.concat(parts,'|')}
end
local function joined(owner,d,source)
    local cfg=integer(source,64) and PlayerConfigurations[source]
    local phase=Get(d.leader,'STATE')
    return Player(owner,true) and cfg and source~=owner and
        cfg:GetLeaderTypeName()==d.leader and cfg:GetCivilizationTypeName()==d.civilization and
        Get(d.leader,'OWNER')==owner and Get(d.leader,'SOURCE')==source and
        Get(d.leader,'EVOLUTION_COST')==d.cost and
        (phase=='GREAT_PERSON' or Get(d.leader,'EVOLUTION_SURCHARGE')==d.cost-1) and
        (phase=='GREAT_PERSON' or phase=='EVOLVING' or phase=='GOVERNOR')
end
local function proof(p,s,owner,d,key,source)
    local app=key=='appointed'; local promotion=not app and d.promotions[key]
    if not app and not promotion then return false end
    if type(p)~='table' or type(p.raw)~='table' then return false end
    local args={owner,d.governor}; if not app then args[3]=promotion.index end
    for i,v in ipairs(args) do
        if type(p.raw[i])~='table' or p.raw[i].type~='number' or p.raw[i].value~=v then return false end
    end
    return p.source==SOURCE and p.context=='Gameplay' and p.accepted==true and p.reason=='acquired' and
        p.owner==owner and p.leader==d.leader and p.sourcePlayer==source and p.binding==s.binding and
        p.governor==d.governor and p.governorHash==d.governorHash and
        p.name==(app and 'GovernorAppointed' or 'GovernorPromoted') and p.arity==#args and
        p.promotion==(app and -1 or promotion.index) and p.promotionHash==(app and -1 or promotion.hash) and
        integer(p.sequence) and p.sequence>0 and p.sequence<=s.sequence and
        integer(p.turn) and p.turn>=s.turn and p.turn<=Game.GetCurrentGameTurn() and
        (p.spent==nil or integer(p.spent))
end
local function readState(ignoreAdmissionSchema)
    local s=Game:GetProperty(STORE); local c=catalog()
    if not c or type(s)~='table' or s.version~=3 or s.source~=SOURCE or s.binding~=c.binding or
       s.baseline~=0 or not integer(s.turn) or s.turn~=0 or not integer(s.sequence) or
       type(s.people)~='table' or type(s.owners)~='table' or #s.owners~=22 or type(s.sources)~='table' or
       type(s.commit)~='table' or s.commit.nativeClosedAll22~=true or s.commit.schema~=3 or
       s.commit.binding~=s.binding or s.commit.context~='Gameplay' or s.commit.turn~=s.turn or
       type(s.receipts)~='table' or #s.receipts>64 or type(s.accounts)~='table' or
       (s.continuityTainted~=false and s.continuityTainted~=true) or
       (not ignoreAdmissionSchema and Game:GetProperty('JAC_PERSONA_ADMISSION_SCHEMA_V1')~=3) then return nil,c end
    local owners={}
    for _,owner in ipairs(s.owners) do if not integer(owner,64) or owners[owner] then return nil,c end; owners[owner]=true end
    local debitSequences={}
    for _,owner in ipairs(s.owners) do
        local a=s.accounts[owner]
        if type(a)~='table' or not integer(a.floor) or not integer(a.floorSerial) or not integer(a.expected) or
           not integer(a.serial,1048577) or type(a.debits)~='table' or #a.debits~=a.serial or
           type(a.byKey)~='table' or (a.tainted~=false and a.tainted~=true) then return nil,c end
        local prior,seen=0,{}
        for serial=1,a.serial do
            local r=a.debits[serial]
            if type(r)~='table' or r.serial~=serial or r.owner~=owner or r.binding~=s.binding or
               r.source~=SOURCE or r.context~='Gameplay' or not integer(r.before) or r.before<prior or
               r.expected~=r.before+1 or not integer(r.sequence) or r.sequence<1 or r.sequence>s.sequence or
               debitSequences[r.sequence] or c.allGovernors[r.governor]~=r.governorType or
               not c.membership[r.governor] or not c.membership[r.governor][r.promotion] or
               not integer(r.floorSerial) or r.floorSerial>a.floorSerial or
               (r.invalidated~=false and r.invalidated~=true) then return nil,c end
            local g=GameInfo.Governors[r.governorType]; local p=c.allPromotions[r.promotion]
            if not g or g.Hash~=r.governorHash or not p or p.Hash~=r.promotionHash or
               p.GovernorPromotionType~=r.promotionType then return nil,c end
            local key=r.governor..'/'..r.promotion
            if seen[key] or a.byKey[key]~=serial then return nil,c end
            local confirmed=r.confirmed
            if confirmed~=nil then
                if type(confirmed)~='table' or r.invalidated or confirmed.source~=SOURCE or confirmed.context~='Gameplay' or
                   confirmed.owner~=owner or confirmed.binding~=s.binding or confirmed.serial~=serial or
                   confirmed.floorSerial~=r.floorSerial or not integer(confirmed.afterSpent) or
                   confirmed.afterSpent~=confirmed.expected or confirmed.expected<r.expected or confirmed.expected>a.expected or
                   not integer(confirmed.sequence) or confirmed.sequence<r.sequence or confirmed.sequence>s.sequence or
                   not integer(confirmed.turn) or confirmed.turn>Game.GetCurrentGameTurn() then return nil,c end
            end
            seen[key]=true; debitSequences[r.sequence]=true; prior=r.expected
        end
        for key in pairs(a.byKey) do if not seen[key] then return nil,c end end
        for key in pairs(a.debits) do if not integer(key,1048577) or key<1 or key>a.serial then return nil,c end end
        if a.expected~=math.max(a.floor,prior) then return nil,c end
    end
    for owner in pairs(s.accounts) do if not owners[owner] then return nil,c end end
    local sourceIDs={}
    for _,d in ipairs(c.rows) do
        local id=s.sources[d.leader]; local cfg=integer(id,64) and PlayerConfigurations[id]
        if not cfg or not owners[id] or sourceIDs[id] or cfg:GetLeaderTypeName()~=d.leader or
           cfg:GetCivilizationTypeName()~=d.civilization then return nil,c end
        sourceIDs[id]=true
    end
    local sequences={}
    for leader,entry in pairs(s.people) do
        local d=c.leaders[leader]
        if not d or type(entry)~='table' or not owners[entry.owner] or
           not integer(entry.sourcePlayer,64) or type(entry.acquired)~='table' or
           (not entry.tainted and entry.sourcePlayer~=s.sources[leader]) then return nil,c end
        for key,p in pairs(entry.acquired) do
            if not proof(p,s,entry.owner,d,key,entry.sourcePlayer) or sequences[p.sequence] then return nil,c end
            sequences[p.sequence]=true
        end
        if entry.tainted~=nil and entry.tainted~=true then return nil,c end
        local r=entry.reservation
        if not entry.tainted then
            local a=s.accounts[entry.owner]
            if type(r)~='table' or r.owner~=entry.owner or r.sourcePlayer~=entry.sourcePlayer or r.binding~=s.binding or
               not integer(r.baseline) or not integer(r.expected) or r.expected<r.baseline or r.expected>a.expected or
               not integer(r.serial) or r.serial>a.serial or not integer(r.floorSerial) or r.floorSerial>a.floorSerial or
               not integer(r.sequence) or r.sequence>s.sequence or
               not integer(r.turn) or r.turn>Game.GetCurrentGameTurn() then return nil,c end
            for key,p in pairs(entry.acquired) do
                if key~='appointed' then
                    local debit=integer(p.spendingSerial) and a.debits[p.spendingSerial]
                    if not debit or p.spendingSerial<=r.serial or debit.expected<=r.expected or
                       debit.sequence>p.sequence or debit.governor~=p.governor or debit.promotion~=p.promotion or
                       p.expectedSpent~=debit.expected then return nil,c end
                end
            end
        end
    end
    return s,c
end
local function state()
    local s,c=readState()
    if not subscribed() or (s and s.continuityTainted) then return nil,c end
    return s,c
end
-- Capture freshness before the existing admission initializer claims schema 3.
-- Settings and a later ready state cannot recreate this lexical candidate.
local candidate
local function fresh()
    if Game:GetProperty(STORE)~=nil or Game.GetCurrentGameTurn()~=0 or
       Game:GetProperty('JAC_PERSONA_ADMISSION_SCHEMA_V1')~=nil or
       Game:GetProperty('JAC_PERSONA_PAID_ASSIGNMENT_SCHEMA')~=nil or
       Game:GetProperty('JAC_TEST853_ADMISSION_ACTIVE')~=nil or
       Game:GetProperty('JAC_TEST853_ADMISSION_MANUAL_APPOINT')~=nil or
       Game:GetProperty('JAC_TEST853_PAID_ASSIGNMENT_V1')~=nil or E.Enabled() then return nil end
    local c=catalog(); local ids=PlayerManager.GetAliveMajorIDs(); local seen,sources={},{}
    if not c or type(ids)~='table' or #ids~=22 then return nil end
    for _,id in ipairs(ids) do
        if not integer(id,64) or seen[id] or not Player(id,true) or spent(id)~=0 then return nil end
        seen[id]=true
        local cfg=PlayerConfigurations[id]; local d=cfg and c.leaders[cfg:GetLeaderTypeName()]
        if not d or sources[d.leader] or cfg:GetCivilizationTypeName()~=d.civilization then return nil end
        sources[d.leader]=id
    end
    for owner=0,63 do
        if Player(owner,false) and (not seen[owner] or spent(owner)~=0) then return nil end
    end
    for _,d in ipairs(c.rows) do
        for _,field in ipairs({'OWNER','SOURCE','STATE','UNIT','EVOLUTION_COST','EVOLUTION_SURCHARGE'}) do
            if Get(d.leader,field)~=nil then return nil end
        end
    end
    local accounts={}
    for _,owner in ipairs(ids) do accounts[owner]={floor=0,floorSerial=0,expected=0,serial=0,debits={},byKey={},tainted=false} end
    return {version=3,source=SOURCE,binding=c.binding,baseline=0,turn=0,continuityTainted=false,
        sequence=0,owners=copy(ids),sources=sources,accounts=accounts,people={},receipts={}}
end
local freshOK,result=pcall(fresh); if freshOK then candidate=result end
local admissionReads={}
function E.NormalAdmissionRead(p,key,value)
    if candidate then
        local owner=p:GetID(); admissionReads[owner]=admissionReads[owner] or {}
        admissionReads[owner][key]={value=value,type=type(value)}
    end
end
function E.NormalCommit(ok,forceClosed)
    if not candidate then return end
    local s=candidate; candidate=nil -- one initialization boundary only
    if ok~=true or forceClosed~=true or not subscribed() or E.Enabled() or Game:GetProperty(STORE)~=nil or
       Game:GetProperty('JAC_PERSONA_ADMISSION_SCHEMA_V1')~=3 then return end
    local c=catalog(); if not c or c.binding~=s.binding or Game.GetCurrentGameTurn()~=0 then return end
    for _,owner in ipairs(s.owners) do
        local reads=admissionReads[owner] or {}
        local cfg=PlayerConfigurations[owner]; local d=cfg and c.leaders[cfg:GetLeaderTypeName()]
        if not d or s.sources[d.leader]~=owner or cfg:GetCivilizationTypeName()~=d.civilization then return end
        local function ack(key,value) local r=reads[key]; return r and r.type=='boolean' and r.value==value end
        if not Player(owner,true) or spent(owner)~=0 or not ack(902000+owner,true) then return end
        for _,d in ipairs(c.rows) do
            for _,field in ipairs({'OWNER','SOURCE','STATE','UNIT','EVOLUTION_COST','EVOLUTION_SURCHARGE'}) do
                if Get(d.leader,field)~=nil then return end
            end
            if not ack(903000+d.governor,true) or
               not ack(901000+d.governor,false) or not ack(900000+d.governor,true) then return end
        end
    end
    s.commit={nativeClosedAll22=true,schema=3,binding=s.binding,context='Gameplay',turn=0}
    Game:SetProperty(STORE,s)
end
-- Only a live lexical Gameplay getter can create a durable debit confirmation.
-- Unexpected excess spending cannot settle earlier pending records.
function E.Confirm(owner)
    if E.Enabled() then return end
    local s=state(); local a=s and s.accounts[owner]; local n=spent(owner)
    if not a or a.tainted or n==nil or n~=a.expected then return end
    local changed=false
    for _,r in ipairs(a.debits) do
        if not r.confirmed and not r.invalidated and r.floorSerial==a.floorSerial then
            r.confirmed={source=SOURCE,context='Gameplay',owner=owner,binding=s.binding,serial=r.serial,
                floorSerial=r.floorSerial,afterSpent=n,expected=a.expected,sequence=s.sequence,turn=Game.GetCurrentGameTurn()}
            changed=true
        end
    end
    if changed then Game:SetProperty(STORE,s) end
end
function E.BeforeEvolution(owner,row)
    if E.Enabled() then return true end
    E.Confirm(owner)
    local s,c=state(); local d=c and row and c.leaders[row.LeaderType]
    local n=spent(owner); local a=s and s.accounts[owner]
    if not s or not d or not a or a.tainted or n==nil or Get(d.leader,'STATE')~='GREAT_PERSON' or
       not joined(owner,d,Get(d.leader,'SOURCE')) then return false end
    -- A successful official debit may precede the aggregate Spent update.
    -- Its still-visible balance cannot fund another person's reservation.
    if n<a.expected then return false end
    local e=s.people[d.leader]
    if e and (e.tainted or next(e.acquired)) then return false end
    if n>a.expected then
        if a.floorSerial>=9007199254740990 then return false end
        -- Preserve confirmed history. A new baseline is only for new work;
        -- every unconfirmed old debit stays unknown, never retroactively paid.
        for _,pending in ipairs(a.debits) do if not pending.confirmed then pending.invalidated=true end end
        a.floorSerial=a.floorSerial+1
    end
    a.floor=math.max(a.floor,n); a.expected=math.max(a.expected,n)
    e=e or {owner=owner,sourcePlayer=Get(d.leader,'SOURCE'),acquired={}}
    e.reservation={owner=owner,sourcePlayer=e.sourcePlayer,binding=s.binding,baseline=n,
        expected=a.expected,serial=a.serial,floorSerial=a.floorSerial,sequence=s.sequence,turn=Game.GetCurrentGameTurn()}
    s.people[d.leader]=e; Game:SetProperty(STORE,s)
    return true
end
local function absence(s,c,owner,d,entry)
    if not entry or not entry.reservation or next(entry.acquired) then return false end
    local a=s.accounts[owner]
    if not a or a.tainted or entry.reservation.floorSerial~=a.floorSerial or spent(owner)~=a.expected then return false end
    for _,other in ipairs(c.rows) do
        if Get(other.leader,'OWNER')==owner and Get(other.leader,'STATE')=='GOVERNOR' then
            local e=s.people[other.leader]
            if not e or e.tainted or not e.reservation or
               not joined(owner,other,e.sourcePlayer) or not e.acquired.appointed then return false end
            for step=1,other.cost do
                local p=e.acquired[step]; local debit=p and a.debits[p.spendingSerial]
                if not debit or not debit.confirmed or debit.invalidated then return false end
            end
        end
    end
    return true
end
function E.NormalPaid(owner,row,cost)
    local s,c=state(); local d=c and row and c.leaders[row.LeaderType]
    if not s or not d or cost~=d.cost or not Player(owner,true) or not s.accounts[owner] then return nil,true end
    local entry=s.people[d.leader]
    if not entry then
        if s.accounts[owner].tainted then return nil,true end
        if Get(d.leader,'OWNER')==nil then return 0,true end
        -- No notifications yet is a full reservation, not a paid receipt.
        if joined(owner,d,Get(d.leader,'SOURCE')) and Get(d.leader,'STATE')=='GREAT_PERSON' then return 0,true end
        return nil,true
    end
    if entry.tainted or entry.owner~=owner or not joined(owner,d,entry.sourcePlayer) then return nil,true end
    local a=entry.acquired
    if not next(a) then
        if s.accounts[owner].tainted then return nil,true end
        if Get(d.leader,'STATE')=='GREAT_PERSON' or
           (Get(d.leader,'STATE')=='EVOLVING' and absence(s,c,owner,d,entry)) then return 0,true end
        return nil,true
    end
    if not a.appointed or not a[1] then return nil,true end
    local paid,missing,pending=0,false,false
    for step=1,d.cost do
        if not a[step] then missing=true
        elseif missing then return nil,true
        else
            local debit=s.accounts[owner].debits[a[step].spendingSerial]
            if debit.confirmed and not debit.invalidated and not pending then paid=paid+1 else pending=true end
        end
    end
    -- Confirm debit visibility for the whole owner's sets. Spending never
    -- allocates a title to a person or supplies a missing success callback.
    local n=spent(owner)
    if paid==0 or n==nil then return nil,true end
    for step=1,paid do
        local confirmation=s.accounts[owner].debits[a[step].spendingSerial].confirmed
        if n<confirmation.afterSpent then return nil,true end
    end
    return paid,true
end
function E.NormalNative(owner,row)
    local s,c=state(); local d=c and row and c.leaders[row.LeaderType]
    if not s or not d or not Player(owner,true) then return nil end
    local paid=E.NormalPaid(owner,row,d.cost)
    if paid==nil then return nil end
    return {appointed=paid>0,base=paid>0,surchargePaid=math.max(0,paid-1),source=SOURCE}
end
function E.Reserved(owner,fallback)
    if E.Enabled() then return (fallback or Rules.ReservedAppointments)(owner) end
    local s,c=state(); local total,uncertain=0,not s
    for row in GameInfo.JAC_LeaderPersonas() do
        if Get(row.LeaderType,'OWNER')==owner and Get(row.LeaderType,'STATE')=='EVOLVING' then
            local cost=Get(row.LeaderType,'EVOLUTION_COST')
            if not integer(cost,4) or cost<1 or cost~=row.EvolutionCost then return math.huge,true end
            local paid=E.NormalPaid(owner,row,cost)
            if paid==nil then uncertain=true; paid=0 end
            total=total+cost-paid
        end
    end
    return total,uncertain
end
local function debit(s,c,owner,governor,promotion,sequence)
    local a=s.accounts[owner]; local g=GameInfo.Governors[governor]; local p=c.allPromotions[promotion]
    if not a or not g or not p or not Player(owner,true) or c.allGovernors[governor]~=g.GovernorType then return nil end
    local key=governor..'/'..promotion; local old=a.byKey[key]
    if old then return a.debits[old] end
    if a.serial>=1048576 or a.expected>=9007199254740990 then return nil end
    local r={source=SOURCE,context='Gameplay',owner=owner,binding=s.binding,sequence=sequence,
        serial=a.serial+1,before=a.expected,expected=a.expected+1,governor=governor,governorType=g.GovernorType,
        governorHash=g.Hash,promotion=promotion,promotionType=p.GovernorPromotionType,promotionHash=p.Hash,
        floorSerial=a.floorSerial,invalidated=false}
    a.serial=r.serial; a.expected=r.expected; a.debits[r.serial]=r; a.byKey[key]=r.serial
    return r
end
local function belongs(c,governor,promotion)
    return c.membership[governor] and c.membership[governor][promotion]==true or false
end
local function officialBase(c,governor)
    return c.baseByGovernor[governor]
end
local function receive(name,...)
    if E.Enabled() then return end
    local s,c=state(); if not s or s.sequence>=9007199254740990 then return end
    local arity=select('#',...); local args={...}; local app=name=='GovernorAppointed'
    local raw={}
    for i=1,math.min(arity,8) do
        local v=args[i]; local t=type(v)
        if t~='number' and t~='boolean' then v=string.sub(tostring(v),1,128) end
        if t=='number' and (v~=v or v==math.huge or v==-math.huge) then v=tostring(v) end
        raw[i]={type=t,value=v}
    end
    s.sequence=s.sequence+1
    local r={source=SOURCE,context='Gameplay',name=name,arity=arity,raw=raw,sequence=s.sequence,
        turn=Game.GetCurrentGameTurn(),accepted=false,reason='arguments',spent=integer(args[1],64) and spent(args[1]) or nil}
    if arity==(app and 2 or 3) and integer(args[1],64) and integer(args[2],256) and (app or integer(args[3],4096)) then
        local owner=args[1]; local d=c.governors[args[2]]
        local key=d and (app and 'appointed' or d.byIndex[args[3]])
        -- Owner consumption is independent of whether this person is joined,
        -- correctly owned or in an evolution reservation. Count every resolved
        -- official success once; contradictory definitions close pending work.
        local spending
        if Player(owner,true) and s.accounts[owner] then
            local promotion=app and officialBase(c,args[2]) or args[3]
            if c.allGovernors[args[2]] and promotion and belongs(c,args[2],promotion) then
                spending=debit(s,c,owner,args[2],promotion,r.sequence)
                if spending then
                    r.spendingSerial=spending.serial; r.expectedSpent=spending.expected
                    r.reason='official-other-spending'
                end
            else
                s.accounts[owner].tainted=true; s.accounts[owner].reason='unresolved-official-spending'
                r.reason='unresolved-official-spending'
            end
        end
        if d and key then
            local source=Get(d.leader,'SOURCE'); local e=s.people[d.leader]
            r.reason='not-joined-or-owner'
            if spending and joined(owner,d,source) and (Get(d.leader,'STATE')=='EVOLVING' or Get(d.leader,'STATE')=='GOVERNOR') and
               e and e.reservation and e.owner==owner and e.sourcePlayer==source and not e.tainted then
                r.owner=owner; r.leader=d.leader; r.sourcePlayer=source; r.binding=s.binding
                r.governor=d.governor; r.governorHash=d.governorHash
                r.promotion=app and -1 or d.promotions[key].index
                r.promotionHash=app and -1 or d.promotions[key].hash
                if not app then
                    -- The first APPOINT may precede BASE. Both refer to the
                    -- same already-recorded consumption, without double debit.
                    if spending.serial<=e.reservation.serial or spending.expected<=e.reservation.expected then
                        e.tainted=true
                    end
                end
                if not e.tainted then
                    r.accepted=true; r.reason=e.acquired[key] and 'duplicate' or 'acquired'
                    if not s.accounts[owner].tainted and not e.acquired[key] then e.acquired[key]=copy(r) end
                end
            elseif Player(owner,true) and Get(d.leader,'OWNER')==nil then
                -- A real pre-join success contradicts fresh absence; reject it
                -- as payment and close this person rather than granting Join.
                s.people[d.leader]={owner=owner,sourcePlayer=owner,acquired={},tainted=true}
            end
        end
    end
    s.receipts[#s.receipts+1]=r; if #s.receipts>64 then table.remove(s.receipts,1) end
    Game:SetProperty(STORE,s)
end
for _,name in ipairs({'GovernorAppointed','GovernorPromoted'}) do
    local ok=pcall(function() Events[name].Add(function(...) receive(name,...) end) end)
    registrations[name]=ok
end
if not subscribed() then
    candidate=nil
    -- Persist the missing interval before normal admission initialization. A
    -- later successful Add cannot make an unobserved appointment absent.
    local s=readState(true)
    if s then
        s.continuityTainted=true
        s.subscriptionGap={source=SOURCE,context='Gameplay',turn=Game.GetCurrentGameTurn(),sequence=s.sequence,
            appointed=registrations.GovernorAppointed,promoted=registrations.GovernorPromoted}
        Game:SetProperty(STORE,s)
    end
end
function E.NormalStatus(owner,leader)
    local s,c=state(); local raw=readState(); local d=c and c.leaders[leader]
    return {source=SOURCE,active=s~=nil,sequence=s and s.sequence,commit=s and copy(s.commit),
        acquired=s and s.people[leader] and copy(s.people[leader].acquired),receipts=s and copy(s.receipts),
        paid=d and E.NormalPaid(owner,Definition(leader),d.cost),reserved=E.Reserved(owner),spent=spent(owner),
        registrations=copy(registrations),continuityTainted=raw and raw.continuityTainted,
        account=s and copy(s.accounts[owner]),reservation=s and s.people[leader] and copy(s.people[leader].reservation)}
end
end
"#;

pub const COMMIT: &str = r#"
-- Only an accepted, naturally read, all-closed initialization can start normal receipts.
do
local publish=SyncAppointmentAdmission
SyncAppointmentAdmission=function(forceClosed,onlyOwner)
    local called,ok=pcall(publish,forceClosed,onlyOwner)
    PersonaEvents.NormalCommit(called and ok,forceClosed)
    if not called then error(ok) end
    return ok
end
end
"#;

pub const NATIVE: &str = r#"local function NativeGovernor(owner, row)
    if PersonaEvents.Enabled() then return PersonaEvents.Native(owner, row); end
    PersonaEvents.Confirm(owner);
    return PersonaEvents.NormalNative(owner, row);
end
"#;
pub const PAYMENT: &str = r#"local function CorePaymentRead(owner, row, cost)
    if PersonaEvents.Enabled() then return PersonaEvents.Paid(owner, row, cost); end
    PersonaEvents.Confirm(owner);
    return PersonaEvents.NormalPaid(owner, row, cost);
end
"#;
