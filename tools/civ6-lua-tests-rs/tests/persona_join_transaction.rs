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

#[test]
fn saved_join_between_person_fields_recovers_in_a_new_lua_context() {
    for field in [
        "SOURCE",
        "NAME",
        "PORTRAIT",
        "PERSONALITY",
        "TRAIT",
        "EVOLUTION_COST",
        "UNIT",
        "OWNER",
        "STATE",
        "MERIT",
    ] {
        let first = scene();
        first.globals().set("CUT_FIELD", field).unwrap();
        exec(
            &first,
            r#"
local get,set=Game.GetProperty,Game.SetProperty
Game.SetProperty=function(self,key,value)
 set(self,key,clone(value))
 if key==KEY..CUT_FIELD and value~=nil and not savedJoin then
  savedJoin={}
  for _,field in ipairs({'SOURCE','NAME','PORTRAIT','PERSONALITY','TRAIT','EVOLUTION_COST','UNIT','OWNER','STATE','MERIT','JOIN_TRANSACTION'}) do
   savedJoin[KEY..field]=clone(get(self,KEY..field))
  end
  savedJoin[STORE]=clone(get(self,STORE))
  for _,unit in Players[0]:GetUnits():Members() do savedPerson=unit:GetID() end
 end
end
assert(api.Join(0,1)=='JOINED' and people()==1)
assert(savedJoin and savedJoin[KEY..CUT_FIELD]~=nil,
 'checkpoint did not cut the actual Join between person writes')
assert(type(savedJoin[KEY..'JOIN_TRANSACTION'])=='table')
"#,
        );
        fn transfer(lua: &Lua, value: mlua::Value) -> mlua::Value {
            match value {
                mlua::Value::String(s) => {
                    mlua::Value::String(lua.create_string(s.as_bytes()).unwrap())
                }
                mlua::Value::Table(table) => {
                    let copy = lua.create_table().unwrap();
                    for pair in table.pairs::<mlua::Value, mlua::Value>() {
                        let (key, value) = pair.unwrap();
                        copy.set(transfer(lua, key), transfer(lua, value)).unwrap();
                    }
                    mlua::Value::Table(copy)
                }
                other => other,
            }
        }
        let second = scene();
        second
            .globals()
            .set(
                "savedJoin",
                transfer(&second, first.globals().get("savedJoin").unwrap()),
            )
            .unwrap();
        second
            .globals()
            .set(
                "savedPerson",
                first.globals().get::<u32>("savedPerson").unwrap(),
            )
            .unwrap();
        exec(
            &second,
            r#"
for key,value in pairs(savedJoin) do Game:SetProperty(key,clone(value)) end
local individual=GameInfo.GreatPersonIndividuals[ROW.PersonType]
Game.GetGreatPeople():GrantPerson(individual.Hash,4000,5000,0,0,false)
assert(Players[0]:GetUnits():FindID(savedPerson),'physical fixture person was not restored')
RESET_CONTEXTS(); assert(loadstring(GAMEPLAY_SOURCE))(); api=ExposedMembers.JAC_LeaderPersona
assert(api.Join(0,1)=='NATIVE_UNAVAILABLE',
 'saved Join field prefix must compensate without repeating its grant')
assert(people()==0 and SPAWNS==1 and DESTROYS==1)
for _,field in ipairs({'SOURCE','NAME','PORTRAIT','PERSONALITY','TRAIT','EVOLUTION_COST','UNIT','OWNER','STATE','MERIT','JOIN_TRANSACTION'}) do
 assert(Game:GetProperty(KEY..field)==nil,'old Join field was not compensated: '..field)
end
assert(Game:GetProperty(STORE).prices[LEADER]==nil and Players[0].spent==0)
assert(api.Join(0,1)=='JOINED' and people()==1 and SPAWNS==2,
 'fresh Join must progress after the saved prefix is compensated')
"#,
        );
    }
}

#[test]
fn complete_join_undo_image_is_saved_before_fields_and_keeps_prior_tables_detached() {
    let lua = scene();
    exec(
        &lua,
        r#"
local set=Game.SetProperty
local initial,full,fieldWrites
fieldWrites=0
Game.SetProperty=function(self,key,value)
 if key==KEY..'JOIN_TRANSACTION' and type(value)=='table' then
  if value.after.UNIT then
   assert(fieldWrites==0,'undo image was saved after a person field changed')
   assert(initial and next(initial.after)==nil,'old saved undo image was mutated through an alias')
   assert(value~=initial and value.before~=initial.before and value.price~=initial.price,
    'persisted undo image shares mutable transaction tables')
   full=value
  else initial=value end
 elseif key:sub(1,#KEY)==KEY and key~=KEY..'JOIN_TRANSACTION' then
  fieldWrites=fieldWrites+1
  assert(full and full.after.UNIT and full.after.STATE=='GREAT_PERSON' and full.after.OWNER==0,
   'person field preceded the complete saved undo image')
 end
 return set(self,key,value)
end
assert(api.Join(0,1)=='JOINED' and people()==1 and fieldWrites==10)
assert(initial and next(initial.after)==nil and full.after.OWNER==0 and full.after.UNIT)
assert(Game:GetProperty(KEY..'JOIN_TRANSACTION')==nil)
assert(api.Join(0,1)=='ALREADY_JOINED' and SPAWNS==1 and DESTROYS==0)
"#,
    );
}

#[test]
fn rejected_complete_join_undo_image_changes_no_fields_and_can_progress_later() {
    for fault in ["reject", "throw_after_write"] {
        let lua = scene();
        lua.globals().set("SAVE_FAULT", fault).unwrap();
        exec(
            &lua,
            r#"
local before=clone(Game:GetProperty(STORE)); local set=Game.SetProperty
local fullWrites,fieldWrites=0,0
Game.SetProperty=function(self,key,value)
 if key==KEY..'JOIN_TRANSACTION' and type(value)=='table' and value.after.UNIT then
  fullWrites=fullWrites+1
  if SAVE_FAULT=='throw_after_write' then set(self,key,clone(value)); error('saved undo writer threw') end
  return
 elseif key:sub(1,#KEY)==KEY and key~=KEY..'JOIN_TRANSACTION' then fieldWrites=fieldWrites+1 end
 return set(self,key,clone(value))
end
assert(api.Join(0,1)=='NATIVE_UNAVAILABLE'); refused(before)
assert(fullWrites>0 and fieldWrites==0,'person fields were published without an accepted undo boundary')
assert(SPAWNS==1 and DESTROYS==1 and Game:GetProperty(KEY..'JOIN_TRANSACTION')==nil)
Game.SetProperty=set
assert(api.Join(0,1)=='JOINED' and people()==1 and SPAWNS==2)
assert(api.Join(0,1)=='ALREADY_JOINED' and SPAWNS==2)
"#,
        );
    }
}

#[test]
fn saved_complete_join_undo_never_replaces_a_foreign_field_or_existing_person() {
    for existing in [false, true] {
        let lua = scene();
        lua.globals().set("EXISTING_PERSON", existing).unwrap();
        exec(
            &lua,
            r#"
local individual=GameInfo.GreatPersonIndividuals[ROW.PersonType]
if EXISTING_PERSON then Game.GetGreatPeople():GrantPerson(individual.Hash,4000,5000,0,0,false) end
local set=Game.SetProperty; local saved
Game.SetProperty=function(self,key,value)
 set(self,key,clone(value))
 if key==KEY..'JOIN_TRANSACTION' and type(value)=='table' and value.after.UNIT and not saved then saved=clone(value) end
end
assert(api.Join(0,1)=='JOINED' and people()==1 and saved)
Game.SetProperty=set
-- Recreate a load cut after NAME, retaining a distinct current value whose
-- owner is outside this transaction. Recovery has no authority over it.
for _,field in ipairs({'SOURCE','NAME','PORTRAIT','PERSONALITY','TRAIT','EVOLUTION_COST','UNIT','OWNER','STATE','MERIT'}) do
 Game:SetProperty(KEY..field,saved.before[field])
end
Game:SetProperty(KEY..'SOURCE',saved.after.SOURCE)
Game:SetProperty(KEY..'NAME','a distinct saved name')
Game:SetProperty(KEY..'JOIN_TRANSACTION',clone(saved))
RESET_CONTEXTS(); assert(loadstring(GAMEPLAY_SOURCE))(); api=ExposedMembers.JAC_LeaderPersona
assert(api.Join(0,1)=='JOIN_ROLLBACK_PENDING')
assert(Game:GetProperty(KEY..'NAME')=='a distinct saved name' and SPAWNS==1)
assert(people()==(EXISTING_PERSON and 1 or 0) and DESTROYS==(EXISTING_PERSON and 0 or 1))
assert(Game:GetProperty(KEY..'JOIN_TRANSACTION')~=nil and Players[0].spent==0)
Game:SetProperty(KEY..'NAME',saved.after.NAME)
assert(api.Join(0,1)=='NATIVE_UNAVAILABLE' and Game:GetProperty(KEY..'JOIN_TRANSACTION')==nil)
assert(Game:GetProperty(KEY..'NAME')==saved.before.NAME)
assert(api.Join(0,1)=='JOINED' and people()==1 and SPAWNS==(EXISTING_PERSON and 1 or 2))
"#,
        );
    }
}

#[test]
fn initial_join_journal_reentry_cannot_compensate_or_replace_its_active_transaction() {
    for timing in ["before_write", "after_write"] {
        let lua = scene();
        lua.globals().set("REENTRY_TIMING", timing).unwrap();
        exec(
            &lua,
            r#"
local set=Game.SetProperty; local entered=false
local function reenter()
 entered=true
 nestedResult=api.Join(0,1)
end
Game.SetProperty=function(self,key,value)
 local first=key==KEY..'JOIN_TRANSACTION' and type(value)=='table' and not value.after.UNIT and not entered
 if first and REENTRY_TIMING=='before_write' then reenter() end
 set(self,key,clone(value))
 if first and REENTRY_TIMING=='after_write' then reenter() end
end
local result=api.Join(0,1)
assert(entered and nestedResult=='NATIVE_UNAVAILABLE',
 'initial journal callback reentered its active Join transaction')
assert(result=='JOINED' and people()==1 and SPAWNS==1 and DESTROYS==0,
 'initial journal reentry prevented or duplicated the original Join')
assert(Game:GetProperty(KEY..'JOIN_TRANSACTION')==nil and Players[0].spent==0)
assert(api.Join(0,1)=='ALREADY_JOINED' and SPAWNS==1 and DESTROYS==0)
"#,
        );
    }
}

#[test]
fn initial_join_journal_failures_release_the_guard_without_regranting() {
    for fault in ["reject", "throw_after_write", "readback_error"] {
        let lua = scene();
        lua.globals().set("INITIAL_FAULT", fault).unwrap();
        exec(
            &lua,
            r#"
local get,set=Game.GetProperty,Game.SetProperty
local readback=false
Game.SetProperty=function(self,key,value)
 if key==KEY..'JOIN_TRANSACTION' and type(value)=='table' then
  if INITIAL_FAULT=='reject' then return end
  set(self,key,clone(value)); readback=true
  if INITIAL_FAULT=='throw_after_write' then error('initial journal writer threw after saving') end
  return
 end
 return set(self,key,value)
end
Game.GetProperty=function(self,key)
 if key==KEY..'JOIN_TRANSACTION' and readback and INITIAL_FAULT=='readback_error' then
  error('initial journal readback unavailable')
 end
 return get(self,key)
end
assert(api.Join(0,1)=='NATIVE_UNAVAILABLE')
assert(people()==0 and SPAWNS==0 and DESTROYS==0 and Players[0].spent==0,
 'initial journal failure reached native grant')
Game.GetProperty,Game.SetProperty=get,set
if Game:GetProperty(KEY..'JOIN_TRANSACTION')~=nil then
 assert(api.Join(0,1)=='NATIVE_UNAVAILABLE' and Game:GetProperty(KEY..'JOIN_TRANSACTION')==nil,
  'saved initial journal could not compensate after storage recovered')
 assert(SPAWNS==0 and DESTROYS==0)
end
assert(api.Join(0,1)=='JOINED' and people()==1 and SPAWNS==1 and DESTROYS==0,
 'initial journal failure left the Join guard permanently busy')
assert(api.Join(0,1)=='ALREADY_JOINED' and SPAWNS==1)
"#,
        );
    }
}
