//! Execute shipped Join with real engine-fixture grants, writes and destruction.
use civ6_lua_tests_rs::read_repo;
use mlua::Lua;

const GP: &str = "poc/JAC_PoC_LeaderPersona/Scripts/JAC_LeaderPersona_Gameplay.lua";
fn scene() -> Lua {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    assert_eq!(
        civ6_lua_tests_rs::repo_root().canonicalize().unwrap(),
        root,
        "shared target test support belongs to another checkout"
    );
    let lua = unsafe { Lua::unsafe_new() };
    for path in [
        "tools/civ6-lua-tests-rs/fixtures/persona/stubs.lua",
        "tools/civ6-lua-tests-rs/fixtures/persona/admission_engine.lua",
        "tools/civ6-lua-tests-rs/fixtures/persona/production_admission_engine.lua",
        "poc/JAC_PoC_LeaderPersona/Shared/JAC_PersonaTreeRules.lua",
    ] {
        lua.load(read_repo(path)).exec().unwrap();
    }
    civ6_lua_tests_rs::persona_engine::prepare(&lua).unwrap();
    lua.load(read_repo(GP)).exec().unwrap();
    civ6_lua_tests_rs::persona_engine::finish_startup(&lua).unwrap();
    lua.globals().set("GAMEPLAY_SOURCE", read_repo(GP)).unwrap();
    exec(
        &lua,
        r#"
api=ExposedMembers.JAC_LeaderPersona; LEADER='LEADER_JAC_DATE_TANEMUNE'
ROW=GameInfo.JAC_LeaderPersonas[LEADER]; KEY='JAC_PERSONA_'..LEADER..'_'
STORE='JAC_PERSONA_GAMEPLAY_ACQUISITIONS_V1'
function clone(v) if type(v)~='table' then return v end local t={} for k,x in pairs(v) do t[k]=clone(x) end return t end
function same(a,b)
 if type(a)~=type(b) then return false end
 if type(a)~='table' then return a==b end
 for k,v in pairs(a) do if not same(v,b[k]) then return false end end
 for k in pairs(b) do if a[k]==nil then return false end end
 return true
end
function people()
 local count=0
 for _,u in Players[0]:GetUnits():Members() do
  local id=u:GetGreatPerson():GetIndividual()
  if GameInfo.GreatPersonIndividuals[id].GreatPersonIndividualType==ROW.PersonType then count=count+1 end
 end
 return count
end
function price_events()
 local names={}
 for i=1,64 do
  local name,value=debug.getupvalue(api.Join,i)
  if not name then break end
  names[#names+1]=tostring(name)
  if name=='PersonaEvents' then return value end
 end
 error('real Join price dependency missing: '..table.concat(names,','))
end
function refused(before)
 assert(people()==0,'refused Join left an unregistered Great Person')
 assert(Game:GetProperty(KEY..'OWNER')==nil and Game:GetProperty(KEY..'STATE')==nil)
 assert(same(before,Game:GetProperty(STORE)),'refused Join changed its saved price contract')
 assert(Players[0].spent==0)
end
"#,
    );
    lua
}
fn exec(lua: &Lua, s: &str) {
    lua.load(s).exec().unwrap();
}

#[test]
fn invalid_legacy_ledger_is_rejected_before_any_new_great_person() {
    let lua = scene();
    exec(
        &lua,
        r#"
assert(api.Join(0,2)=='JOINED')
local other='JAC_PERSONA_LEADER_JAC_ASHINA_MORIKIYO_'
local otherID=Game:GetProperty(other..'UNIT')
local s=Game:GetProperty(STORE); s.priceVersion=nil; s.prices=nil; Game:SetProperty(STORE,s)
Game:SetProperty(other..'EVOLUTION_COST',4)
local before=clone(Game:GetProperty(STORE)); local spawned=SPAWNS
assert(api.Join(0,1)=='NATIVE_UNAVAILABLE'); refused(before)
assert(SPAWNS==spawned and Players[0]:GetUnits():FindID(otherID))
assert(Game:GetProperty(other..'EVOLUTION_COST')==4,'damaged old amount was repaired')
assert(api.Join(0,1)=='NATIVE_UNAVAILABLE'); refused(before); assert(SPAWNS==spawned)
"#,
    );
}

#[test]
fn source_mismatch_and_corrupt_store_never_spawn_or_change_contracts() {
    for fault in [
        "local s=Game:GetProperty(STORE); s.sources[LEADER]=2; Game:SetProperty(STORE,s)",
        "local s=Game:GetProperty(STORE); s.accounts[0].expected=7; Game:SetProperty(STORE,s)",
    ] {
        let lua = scene();
        exec(&lua, fault);
        exec(
            &lua,
            r#"
local before=clone(Game:GetProperty(STORE))
assert(api.Join(0,1)=='NATIVE_UNAVAILABLE'); refused(before)
assert(api.Join(0,1)=='NATIVE_UNAVAILABLE'); refused(before); assert(SPAWNS==0)
"#,
        );
    }
}

#[test]
fn successful_spawn_followed_by_price_rejection_is_physically_rolled_back() {
    let lua = scene();
    exec(
        &lua,
        r#"
local before=clone(Game:GetProperty(STORE)); local e=price_events(); local record=e.RecordJoinPrice
local grant=Game.GetGreatPeople
Game.GetGreatPeople=function()
 local g=grant(); local create=g.GrantPerson
 g.GrantPerson=function(self,...)
  create(self,...)
  e.RecordJoinPrice=function() return false end
 end
 return g
end
assert(api.Join(0,1)=='NATIVE_UNAVAILABLE'); refused(before)
assert(SPAWNS==1 and DESTROYS==1,'real generated unit was not removed')
Game.GetGreatPeople=grant; e.RecordJoinPrice=record
assert(api.Join(0,1)=='JOINED' and people()==1 and SPAWNS==2)
local id=Game:GetProperty(KEY..'UNIT'); local committed=clone(Game:GetProperty(STORE))
assert(api.Join(0,1)=='ALREADY_JOINED' and people()==1 and SPAWNS==2)
assert(Game:GetProperty(KEY..'UNIT')==id and same(committed,Game:GetProperty(STORE)))
"#,
    );
}

#[test]
fn spawn_failure_and_existing_person_preserve_identity_and_price() {
    let lua = scene();
    exec(
        &lua,
        r#"
local before=clone(Game:GetProperty(STORE)); FAIL_SPAWN=true
assert(api.Join(0,1)=='SPAWN_FAILED'); refused(before)
FAIL_SPAWN=false
local individual=GameInfo.GreatPersonIndividuals[ROW.PersonType]
Game.GetGreatPeople():GrantPerson(individual.Hash,4000,5000,0,0,false)
local unit; for _,u in Players[0]:GetUnits():Members() do unit=u end
unit.GetName=function() return 'kept engine name' end
local id=unit:GetID(); local spawned=SPAWNS
assert(api.Join(0,1)=='JOINED' and SPAWNS==spawned and people()==1)
assert(Game:GetProperty(KEY..'UNIT')==id and unit:GetName()=='kept engine name')
assert(api.Join(0,1)=='ALREADY_JOINED' and Game:GetProperty(KEY..'UNIT')==id)
"#,
    );
}

#[test]
fn store_and_field_write_failures_restore_price_and_remove_only_created_unit() {
    for fault in ["reject_price", "throw_after_price", "reject_owner"] {
        let lua = scene();
        lua.globals().set("FAULT", fault).unwrap();
        exec(
            &lua,
            r#"
local before=clone(Game:GetProperty(STORE)); local set=Game.SetProperty
Game.SetProperty=function(self,key,value)
 if key==STORE and type(value)=='table' and value.prices and value.prices[LEADER] then
  if FAULT=='reject_price' then return end
  if FAULT=='throw_after_price' then set(self,key,value); error('committed price writer threw') end
 end
 if key==KEY..'OWNER' and value~=nil and FAULT=='reject_owner' then return end
 return set(self,key,value)
end
assert(api.Join(0,1)=='NATIVE_UNAVAILABLE'); refused(before)
assert(SPAWNS==1 and DESTROYS==1)
Game.SetProperty=set
assert(api.Join(0,1)=='JOINED' and people()==1)
assert(api.Join(0,1)=='ALREADY_JOINED' and SPAWNS==2)
"#,
        );
    }
}

#[test]
fn refused_destroy_is_tracked_and_retries_compensate_without_second_grant() {
    let lua = scene();
    exec(
        &lua,
        r#"
local before=clone(Game:GetProperty(STORE)); local e=price_events(); local record=e.RecordJoinPrice
e.RecordJoinPrice=function() return false end; FAIL_DESTROY=true
assert(api.Join(0,1)=='JOIN_ROLLBACK_PENDING')
assert(people()==1 and SPAWNS==1 and same(before,Game:GetProperty(STORE)))
assert(type(Game:GetProperty(KEY..'JOIN_TRANSACTION'))=='table','failed physical rollback was untracked')
RESET_CONTEXTS(); assert(loadstring(GAMEPLAY_SOURCE))(); api=ExposedMembers.JAC_LeaderPersona
assert(api.Join(0,1)=='JOIN_ROLLBACK_PENDING'); assert(people()==1 and SPAWNS==1)
FAIL_DESTROY=false; e.RecordJoinPrice=record
assert(api.Join(0,1)=='NATIVE_UNAVAILABLE'); refused(before); assert(SPAWNS==1)
assert(Game:GetProperty(KEY..'JOIN_TRANSACTION')==nil)
assert(api.Join(0,1)=='JOINED' and SPAWNS==2 and people()==1)
"#,
    );
}

#[test]
fn rejection_when_adopting_an_existing_person_never_destroys_or_renames_it() {
    let lua = scene();
    exec(
        &lua,
        r#"
local individual=GameInfo.GreatPersonIndividuals[ROW.PersonType]
Game.GetGreatPeople():GrantPerson(individual.Hash,4000,5000,0,0,false)
local unit; for _,u in Players[0]:GetUnits():Members() do unit=u end
unit.GetName=function() return 'original engine name' end
local before=clone(Game:GetProperty(STORE)); local e=price_events(); local record=e.RecordJoinPrice
e.RecordJoinPrice=function() return false end
assert(api.Join(0,1)=='NATIVE_UNAVAILABLE')
assert(people()==1 and SPAWNS==1 and DESTROYS==0 and unit:GetName()=='original engine name')
assert(same(before,Game:GetProperty(STORE)) and Game:GetProperty(KEY..'OWNER')==nil)
e.RecordJoinPrice=record
assert(api.Join(0,1)=='JOINED' and Game:GetProperty(KEY..'UNIT')==unit:GetID() and SPAWNS==1)
"#,
    );
}

#[test]
fn reentrant_grant_and_throw_after_creation_do_not_duplicate_or_leak_persons() {
    for fault in ["reentry", "throw"] {
        let lua = scene();
        lua.globals().set("FAULT", fault).unwrap();
        exec(
            &lua,
            r#"
local before=clone(Game:GetProperty(STORE)); local grant=Game.GetGreatPeople
Game.GetGreatPeople=function()
 local g=grant(); local create=g.GrantPerson
 g.GrantPerson=function(self,...)
  if FAULT=='reentry' then assert(api.Join(0,1)=='NATIVE_UNAVAILABLE') end
  create(self,...)
  if FAULT=='throw' then error('engine grant threw after creating its unit') end
 end
 return g
end
local result=api.Join(0,1)
if FAULT=='throw' then
 assert(result=='SPAWN_FAILED'); refused(before); assert(SPAWNS==1 and DESTROYS==1)
else
 assert(result=='JOINED' and people()==1 and SPAWNS==1)
 assert(api.Join(0,1)=='ALREADY_JOINED' and SPAWNS==1)
end
"#,
        );
    }
}

#[test]
fn corrupt_rollback_price_image_never_rewrites_another_person_contract() {
    let lua = scene();
    exec(
        &lua,
        r#"
assert(api.Join(0,2)=='JOINED')
local other='LEADER_JAC_ASHINA_MORIKIYO'; local otherID=Game:GetProperty('JAC_PERSONA_'..other..'_UNIT')
local set=Game.SetProperty
Game.SetProperty=function(self,key,value)
 if key==KEY..'OWNER' and value~=nil then return end
 if key==STORE and type(value)=='table' and value.prices and not value.prices[LEADER] then return end
 return set(self,key,value)
end
assert(api.Join(0,1)=='JOIN_ROLLBACK_PENDING' and people()==0)
Game.SetProperty=set
local valid=clone(Game:GetProperty(STORE)); local spawned=SPAWNS
local pending=clone(Game:GetProperty(KEY..'JOIN_TRANSACTION'))
pending.price.before.prices[other].cost=4; Game:SetProperty(KEY..'JOIN_TRANSACTION',pending)
local result=api.Join(0,1)
assert(same(valid,Game:GetProperty(STORE)),'damaged undo image corrupted another persons saved price')
assert(result=='JOIN_ROLLBACK_PENDING' and people()==0 and SPAWNS==spawned)
assert(Players[0]:GetUnits():FindID(otherID),'undo image removed another persons Great Person')
assert(api.Join(0,1)=='JOIN_ROLLBACK_PENDING' and SPAWNS==spawned)
assert(same(valid,Game:GetProperty(STORE)))
"#,
    );
}
