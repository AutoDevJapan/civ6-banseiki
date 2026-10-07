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
local function configuredStartTurn()
    if not GameConfiguration or type(GameConfiguration.GetStartTurn)~='function' then return nil end
    local ok,value=pcall(GameConfiguration.GetStartTurn)
    if ok and integer(value) then return value end
end
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
    local effectful={}
    if GameInfo.GovernorPromotionModifiers then
        for modifier in GameInfo.GovernorPromotionModifiers() do effectful[modifier.GovernorPromotionType]=true end
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
        -- Keep every available payment definition. Repricing may shorten the
        -- new price, but saved people still need their original prefix.
        for step=0,2 do
            local name=row.BasePromotionType..(step==0 and '' or '_TITLE_'..step)
            local p=GameInfo.GovernorPromotions[name]
            if not p and step>=d.cost then break end
            if not p or p.GovernorPromotionType~=name or not indices[p.Index] or
               indices[p.Index].GovernorPromotionType~=name or types[name].Index~=p.Index or
               not hash(p.Hash) or promotions[p.Index] or not membership[g.Index] or
               not membership[g.Index][p.Index] or (step==0 and baseByGovernor[g.Index]~=p.Index) or
               (step>0 and effectful[name]) then return nil end
            promotions[p.Index]=true
            d.promotions[step+1]={index=p.Index,hash=p.Hash,name=name}; d.byIndex[p.Index]=step+1
        end
        rows[#rows+1]=d; leaders[l]=d; governors[g.Index]=d; hashes[g.Hash]=true
        people[person.Index]=true; personHashes[person.Hash]=true
    end
    if #rows~=22 then return nil end
    table.sort(rows,function(a,b) return a.governor<b.governor end)
    table.sort(setBinding)
    local function setName(d,p)
        return table.concat({d.governorType,d.governor,d.governorHash,p.name,p.index,p.hash,p==d.promotions[1] and 1 or 0},'/')
    end
    local function bind(costs,sets)
        local parts={SOURCE}
        for _,set in ipairs(sets or setBinding) do parts[#parts+1]=#set..':'..set end
        for _,d in ipairs(rows) do
            local cost=costs and costs[d.leader] or d.cost
            if not integer(cost,4) or cost<1 or not d.promotions[cost] then return nil end
            for _,v in ipairs({d.leader,d.governorType,d.governor,d.governorHash,d.personType,d.person,d.personHash,d.civilization,cost}) do
                local value=tostring(v); parts[#parts+1]=#value..':'..value
            end
            for step=1,cost do
                local p=d.promotions[step]
                for _,v in ipairs({p.name,p.index,p.hash}) do local value=tostring(v); parts[#parts+1]=#value..':'..value end
            end
        end
        return table.concat(parts,'|')
    end
    local function boundPrices(binding)
        if type(binding)~='string' then return nil end
        local parts={}; for part in string.gmatch(binding,'[^|]+') do parts[#parts+1]=part end
        local first=#rows[1].leader..':'..rows[1].leader
        local at
        for i=2,#parts do if parts[i]==first then at=i-1; break end end
        if not at then return nil end
        local historical,present,current={},{},{}
        for _,set in ipairs(setBinding) do current[set]=true end
        for i=2,at do
            local length,set=string.match(parts[i],'^(%d+):(.*)$')
            if not set or tonumber(length)~=#set or not current[set] or present[set] then return nil end
            historical[#historical+1]=set; present[set]=true
        end
        local costs={}
        for _,d in ipairs(rows) do
            local part=parts[at+9]
            local cost=part and string.match(part,'^1:([123])$')
            cost=cost and tonumber(cost)
            if not cost then return nil end
            costs[d.leader]=cost; at=at+9+3*cost
        end
        -- Replay the entire original catalog. A price increase may append only
        -- canonical, effect-free payment memberships beyond its old prefix.
        -- Every existing identity, Index, Hash and membership remains exact.
        if at~=#parts or bind(costs,historical)~=binding then return nil end
        local additions={}
        for _,d in ipairs(rows) do
            for step,p in ipairs(d.promotions) do
                local set=setName(d,p)
                if step<=costs[d.leader] then
                    if not present[set] then return nil end
                else additions[set]=true end
            end
        end
        for _,set in ipairs(setBinding) do if not present[set] and not additions[set] then return nil end end
        return costs
    end
    return {rows=rows,leaders=leaders,governors=governors,allGovernors=governorIndices,
        allPromotions=indices,membership=membership,baseByGovernor=baseByGovernor,binding=bind(),boundPrices=boundPrices}
end
local function priceContract(p,d)
    return type(p)=='table' and p.version==1 and p.leader==d.leader and
        integer(p.owner,64) and integer(p.sourcePlayer,64) and p.owner~=p.sourcePlayer and
        p.governorType==d.governorType and p.governor==d.governor and p.governorHash==d.governorHash and
        p.personType==d.personType and p.person==d.person and p.personHash==d.personHash and p.baseType==d.baseType and
        integer(p.cost,4) and p.cost>=1 and d.promotions[p.cost]~=nil
end
local function joinedCost(d)
    local cost=d and d.joinCost
    local phase=d and Get(d.leader,'STATE')
    if not integer(cost,4) or cost<1 or Get(d.leader,'EVOLUTION_COST')~=cost or
       (phase~='GREAT_PERSON' and Get(d.leader,'EVOLUTION_SURCHARGE')~=cost-1) then return nil end
    return cost
end
local function joined(owner,d,source)
    local cfg=integer(source,64) and PlayerConfigurations[source]
    local phase=Get(d.leader,'STATE')
    return Player(owner,true) and cfg and source~=owner and
        cfg:GetLeaderTypeName()==d.leader and cfg:GetCivilizationTypeName()==d.civilization and
        Get(d.leader,'OWNER')==owner and Get(d.leader,'SOURCE')==source and
        joinedCost(d)~=nil and
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
local function readState(ignoreAdmissionSchema,snapshot)
    local s=snapshot or Game:GetProperty(STORE); local c=catalog()
    local prices=c and type(s)=='table' and c.boundPrices(s.binding)
    if not prices or s.version~=3 or s.source~=SOURCE or
       s.baseline~=0 or not integer(s.turn) or s.turn~=configuredStartTurn() or not integer(s.sequence) or
       type(s.people)~='table' or type(s.owners)~='table' or #s.owners~=22 or type(s.sources)~='table' or
       type(s.commit)~='table' or s.commit.nativeClosedAll22~=true or s.commit.schema~=3 or
       s.commit.binding~=s.binding or s.commit.context~='Gameplay' or s.commit.turn~=s.turn or
       type(s.receipts)~='table' or #s.receipts>64 or type(s.accounts)~='table' or
       (s.continuityTainted~=false and s.continuityTainted~=true) or
       (not ignoreAdmissionSchema and Game:GetProperty('JAC_PERSONA_ADMISSION_SCHEMA_V1')~=3) then return nil,c end
    if s.priceVersion~=nil then
        if s.priceVersion~=1 or type(s.prices)~='table' then return nil,c end
        local bindings={[s.binding]=prices}
        for leader,p in pairs(s.prices) do
            local d=c.leaders[leader]
            if not d or not priceContract(p,d) or p.sourcePlayer~=s.sources[leader] then return nil,c end
            if type(p.binding)~='string' then return nil,c end
            local committed=bindings[p.binding]
            if not committed then committed=c.boundPrices(p.binding); bindings[p.binding]=committed end
            if not committed or committed[leader]~=p.cost then return nil,c end
        end
    elseif s.prices~=nil then return nil,c end
    for _,d in ipairs(c.rows) do
        local owner=Get(d.leader,'OWNER')
        local p=s.prices and s.prices[d.leader]
        if owner~=nil then
            local source=Get(d.leader,'SOURCE')
            if s.prices then
                if not p or p.owner~=owner or p.sourcePlayer~=source then return nil,c end
                d.joinCost=p.cost
            else
                -- d21 saves have independent prices in the committed binding.
                -- Do not infer a missing or reduced amount from current data.
                d.joinCost=prices[d.leader]
            end
        end
    end
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
function E.CatalogCost(row)
    if not row or not integer(row.EvolutionCost,4) or row.EvolutionCost<1 then return nil end
    local leader=row.LeaderType; local owner=Get(leader,'OWNER')
    if owner==nil then return row.EvolutionCost end
    if E.Enabled() then
        local p=Get(leader,'PRICE_CONTRACT')
        if type(p)~='table' or p.version~=1 or p.leader~=leader or p.owner~=owner or
           p.sourcePlayer~=Get(leader,'SOURCE') or p.governorType~=row.GovernorType or
           p.personType~=row.PersonType or p.baseType~=row.BasePromotionType or
           not integer(p.cost,4) or p.cost<1 then return nil end
        return p.cost
    end
    local s,c=state(); local d=s and c.leaders[leader]
    local source=Get(leader,'SOURCE'); local cfg=integer(source,64) and PlayerConfigurations[source]
    if d and d.joinCost and Player(owner,true) and cfg and source~=owner and
       cfg:GetLeaderTypeName()==leader and cfg:GetCivilizationTypeName()==d.civilization then return d.joinCost end
end
function E.Cost(row)
    local cost=E.CatalogCost(row)
    if not cost then return nil end
    local leader=row.LeaderType
    if Get(leader,'OWNER')==nil then return cost end
    local phase=Get(leader,'STATE')
    if Get(leader,'EVOLUTION_COST')~=cost or
       (phase~='GREAT_PERSON' and Get(leader,'EVOLUTION_SURCHARGE')~=cost-1) or
       (phase~='GREAT_PERSON' and phase~='EVOLVING' and phase~='GOVERNOR') then return nil end
    return cost
end
local function same(a,b)
    if type(a)~=type(b) then return false end
    if type(a)~='table' then return a==b end
    for key,value in pairs(a) do if not same(value,b[key]) then return false end end
    for key in pairs(b) do if a[key]==nil then return false end end
    return true
end
function E.PrepareJoinPrice(owner,source,row)
    if not row or not integer(row.EvolutionCost,4) or row.EvolutionCost<1 or Get(row.LeaderType,'OWNER')~=nil then return nil end
    local cfg=integer(source,64) and PlayerConfigurations[source]
    if not Player(owner,true) or not cfg or source==owner or cfg:GetLeaderTypeName()~=row.LeaderType or
       cfg:GetCivilizationTypeName()~=row.CivilizationType then return nil end
    if E.Enabled() then
        local before=Get(row.LeaderType,'PRICE_CONTRACT')
        local after={version=1,leader=row.LeaderType,owner=owner,sourcePlayer=source,
            governorType=row.GovernorType,personType=row.PersonType,baseType=row.BasePromotionType,cost=row.EvolutionCost}
        if before~=nil and not same(before,after) then return nil end
        return {leader=row.LeaderType,owner=owner,source=source,key=Key(row.LeaderType,'PRICE_CONTRACT'),before=copy(before),after=after}
    end
    local s,c=state(); local d=s and c.leaders[row.LeaderType]
    if not d or s.sources[d.leader]~=source or not s.accounts[owner] or s.people[d.leader] then return nil end
    local before=copy(s); s=copy(s) -- preparation must never mutate a Game table
    if not s.prices then
        -- Import only original joining prices that still match the d21
        -- commitment and the joined identities; never repair a damaged amount.
        s.prices={}; s.priceVersion=1
        for _,prior in ipairs(c.rows) do
            local who=Get(prior.leader,'OWNER')
            if who~=nil then
                if not joined(who,prior,Get(prior.leader,'SOURCE')) then return nil end
                s.prices[prior.leader]={version=1,leader=prior.leader,owner=who,sourcePlayer=Get(prior.leader,'SOURCE'),
                    binding=s.binding,
                    governorType=prior.governorType,governor=prior.governor,governorHash=prior.governorHash,
                    personType=prior.personType,person=prior.person,personHash=prior.personHash,baseType=prior.baseType,cost=prior.joinCost}
            end
        end
    end
    local existing=s.prices[d.leader]
    local contract={version=1,leader=d.leader,owner=owner,sourcePlayer=source,
        binding=c.binding,
        governorType=d.governorType,governor=d.governor,governorHash=d.governorHash,
        personType=d.personType,person=d.person,personHash=d.personHash,baseType=d.baseType,cost=row.EvolutionCost}
    if existing and not same(existing,contract) then return nil end
    s.prices[d.leader]=contract
    return {leader=row.LeaderType,owner=owner,source=source,key=STORE,before=before,after=s}
end
function E.RecordJoinPrice(owner,source,row,plan)
    local checked=E.PrepareJoinPrice(owner,source,row)
    if not plan or not checked or plan.key~=checked.key or
       not same(plan.before,checked.before) or not same(plan.after,checked.after) then return false end
    Game:SetProperty(plan.key,copy(plan.after))
    return same(Game:GetProperty(plan.key),plan.after)
end
function E.UndoJoinPrice(plan)
    if type(plan)~='table' or not Definition(plan.leader) or not integer(plan.owner,64) or
       not integer(plan.source,64) or plan.owner==plan.source or
       (plan.key~=STORE and plan.key~=Key(plan.leader,'PRICE_CONTRACT')) then return false end
    if plan.key==STORE then
        if type(plan.before)~='table' or type(plan.after)~='table' then return false end
        local before,after=copy(plan.before),copy(plan.after)
        before.prices=nil; before.priceVersion=nil; after.prices=nil; after.priceVersion=nil
        if not same(before,after) then return false end -- undo has no authority over spending/receipts/sources
        local prior,oldCatalog=readState(false,plan.before)
        local nextState=readState(false,plan.after)
        local target=nextState and nextState.prices and nextState.prices[plan.leader]
        if not prior or not target or target.owner~=plan.owner or target.sourcePlayer~=plan.source then return false end
        if prior.prices then
            for leader,p in pairs(prior.prices) do if not same(p,nextState.prices[leader]) then return false end end
            for leader,p in pairs(nextState.prices) do
                if leader~=plan.leader and not same(p,prior.prices[leader]) then return false end
            end
        else
            -- A legacy import may add only the existing joining contracts,
            -- preserving their original binding and amount.
            for leader,p in pairs(nextState.prices) do
                if leader~=plan.leader then
                    local d=oldCatalog.leaders[leader]
                    if not d or p.binding~=prior.binding or p.cost~=d.joinCost or
                       not joined(p.owner,d,p.sourcePlayer) then return false end
                end
            end
        end
    else
        local row=Definition(plan.leader); local p=plan.after
        if type(p)~='table' or not integer(p.cost,4) or p.cost<1 or
           not same(p,{version=1,leader=plan.leader,owner=plan.owner,sourcePlayer=plan.source,
              governorType=row.GovernorType,personType=row.PersonType,baseType=row.BasePromotionType,cost=p.cost}) or
           (plan.before~=nil and not same(plan.before,p)) then return false end
    end
    local current=Game:GetProperty(plan.key)
    if same(current,plan.before) then return true end
    if not same(current,plan.after) then return false end -- never repair a damaged/foreign ledger
    Game:SetProperty(plan.key,copy(plan.before))
    return same(Game:GetProperty(plan.key),plan.before)
end
-- Capture freshness before the existing admission initializer claims schema 3.
-- Settings and a later ready state cannot recreate this lexical candidate.
local candidate
local initialization={stage='candidate',turn=Game.GetCurrentGameTurn()}
if GameConfiguration and type(GameConfiguration.GetStartTurn)=='function' then
    local ok,value=pcall(GameConfiguration.GetStartTurn)
    initialization.startTurnReadable=ok; if ok then initialization.startTurn=value end
end
local function fresh()
    initialization.stage='fresh_preconditions'
    local startTurn=configuredStartTurn()
    if startTurn==nil or Game:GetProperty(STORE)~=nil or Game.GetCurrentGameTurn()~=startTurn or
       Game:GetProperty('JAC_PERSONA_ADMISSION_SCHEMA_V1')~=nil or
       Game:GetProperty('JAC_PERSONA_PAID_ASSIGNMENT_SCHEMA')~=nil or
       Game:GetProperty('JAC_TEST853_ADMISSION_ACTIVE')~=nil or
       Game:GetProperty('JAC_TEST853_ADMISSION_MANUAL_APPOINT')~=nil or
       Game:GetProperty('JAC_TEST853_PAID_ASSIGNMENT_V1')~=nil or E.Enabled() then return nil end
    initialization.stage='fresh_catalog'
    local c=catalog(); initialization.catalogValid=c~=nil
    local ids=PlayerManager.GetAliveMajorIDs(); local seen,sources={},{}
    initialization.ownerCount=type(ids)=='table' and #ids or -1
    if not c or type(ids)~='table' or #ids~=22 then return nil end
    for _,id in ipairs(ids) do
        initialization.stage='fresh_owner'; initialization.owner=id
        if not integer(id,64) or seen[id] or not Player(id,true) or spent(id)~=0 then return nil end
        seen[id]=true
        local cfg=PlayerConfigurations[id]; local d=cfg and c.leaders[cfg:GetLeaderTypeName()]
        if not d or sources[d.leader] or cfg:GetCivilizationTypeName()~=d.civilization then return nil end
        sources[d.leader]=id
    end
    for owner=0,63 do
        initialization.stage='fresh_other_owner'; initialization.owner=owner
        if Player(owner,false) and (not seen[owner] or spent(owner)~=0) then return nil end
    end
    for _,d in ipairs(c.rows) do
        initialization.stage='fresh_person'; initialization.leader=d.leader
        for _,field in ipairs({'OWNER','SOURCE','STATE','UNIT','EVOLUTION_COST','EVOLUTION_SURCHARGE'}) do
            if Get(d.leader,field)~=nil then return nil end
        end
    end
    local accounts={}
    initialization.stage='candidate_ready'; initialization.owner=nil; initialization.leader=nil
    for _,owner in ipairs(ids) do accounts[owner]={floor=0,floorSerial=0,expected=0,serial=0,debits={},byKey={},tainted=false} end
    return {version=3,source=SOURCE,binding=c.binding,baseline=0,turn=startTurn,continuityTainted=false,
        sequence=0,owners=copy(ids),sources=sources,accounts=accounts,people={},receipts={},priceVersion=1,prices={}}
end
local freshOK,result=pcall(fresh); initialization.candidateCallOK=freshOK
if freshOK then candidate=result else initialization.error=tostring(result) end
initialization.candidate=candidate~=nil
local admissionReads={}
function E.NormalAdmissionRead(p,key,value)
    if candidate then
        local owner=p:GetID(); admissionReads[owner]=admissionReads[owner] or {}
        admissionReads[owner][key]={value=value,type=type(value)}
    end
end
function E.NormalCommit(ok,forceClosed)
    if not candidate then return end
    initialization.stage='commit_preconditions'; initialization.commitOK=ok; initialization.forceClosed=forceClosed
    local s=candidate; candidate=nil -- one initialization boundary only
    if ok~=true or forceClosed~=true or not subscribed() or E.Enabled() or Game:GetProperty(STORE)~=nil or
       Game:GetProperty('JAC_PERSONA_ADMISSION_SCHEMA_V1')~=3 then return end
    initialization.stage='commit_catalog'
    local c=catalog(); if not c or c.binding~=s.binding or s.turn~=configuredStartTurn() or Game.GetCurrentGameTurn()~=s.turn then return end
    for _,owner in ipairs(s.owners) do
        initialization.stage='commit_owner'; initialization.owner=owner
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
    s.commit={nativeClosedAll22=true,schema=3,binding=s.binding,context='Gameplay',turn=s.turn}
    Game:SetProperty(STORE,s)
    initialization.stage='committed'; initialization.owner=nil
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
            for step=1,joinedCost(other) do
                local p=e.acquired[step]; local debit=p and a.debits[p.spendingSerial]
                if not debit or not debit.confirmed or debit.invalidated then return false end
            end
        end
    end
    return true
end
function E.NormalPaid(owner,row,cost)
    local s,c=state(); local d=c and row and c.leaders[row.LeaderType]
    if not s or not d or not Player(owner,true) or not s.accounts[owner] then return nil,true end
    local expected=Get(d.leader,'OWNER')==nil and d.cost or joinedCost(d)
    if cost~=expected then return nil,true end
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
    for step=1,cost do
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
    local cost=Get(d.leader,'OWNER')==nil and d.cost or joinedCost(d)
    local paid=E.NormalPaid(owner,row,cost)
    if paid==nil then return nil end
    return {appointed=paid>0,base=paid>0,surchargePaid=math.max(0,paid-1),source=SOURCE}
end
function E.Reserved(owner,fallback)
    if E.Enabled() then return (fallback or Rules.ReservedAppointments)(owner) end
    local s,c=state(); local total,uncertain=0,not s
    for row in GameInfo.JAC_LeaderPersonas() do
        if Get(row.LeaderType,'OWNER')==owner and Get(row.LeaderType,'STATE')=='EVOLVING' then
            local cost=E.Cost(row)
            if not cost then return math.huge,true end
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
        if key and not app and key>(joinedCost(d) or d.cost) then key=nil end
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
    return {source=SOURCE,active=s~=nil,initialization=copy(initialization),sequence=s and s.sequence,commit=s and copy(s.commit),
        acquired=s and s.people[leader] and copy(s.people[leader].acquired),receipts=s and copy(s.receipts),
        paid=d and E.NormalPaid(owner,Definition(leader),Get(leader,'OWNER')==nil and d.cost or joinedCost(d)),reserved=E.Reserved(owner),spent=spent(owner),
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
