use civ6_lua_tests_rs::{new_lua, read_repo};

fn fixture(replacement_owner: i32) -> mlua::Lua {
    let lua = new_lua();
    lua.load(
        r#"
A={};R={GOVERNANCE_KEY='governance'}
Multiplayer=function() return false end
Log=function() end
Game={GetCurrentGameTurn=function() return 15 end}
Store={Publication=1,OldCalls=0,NewCalls=0}
local function city(current,old)
 return {Current=current,
  GetOwner=function() return old and 0 or ReplacementOwner end,
  GetID=function() return 22 end,GetX=function() return 5 end,GetY=function() return 6 end,
  SetProperty=function(_,key,g)
   assert(key==R.GOVERNANCE_KEY)
   Store.Governance=g
   if g.ImmediatePending.Issued and not Store.Replaced then
    Store.Replaced=true;Store.Current=Store.Replacement
   end
  end,
  ChangeLoyalty=function(self,delta)
   assert(delta==-5)
   if old then Store.OldCalls=Store.OldCalls+1 else Store.NewCalls=Store.NewCalls+1 end
   self.Current=self.Current+delta;Store.Publication=Store.Publication+1
  end}
end
Store.Original=city(100,true);Store.Replacement=city(100,false);Store.Current=Store.Original
CityManager={GetCityAt=function(x,y) assert(x==5 and y==6);return Store.Current end}
ExposedMembers={JAC_HeroReadAPI={
 GetPublication=function() return Store.Publication end,
 GetObservation=function()
  Store.Publication=Store.Publication+1
  return {Owner=Store.Current:GetOwner(),ID=22,X=5,Y=6,Turn=15,
   Publication=Store.Publication,Value=Store.Current.Current}
 end}}
Pending={};Governance={ID='tx',From=1,To=0,ImmediatePending=Pending}
"#,
    )
    .exec()
    .unwrap();
    lua.globals()
        .set("ReplacementOwner", replacement_owner)
        .unwrap();
    let source = read_repo("poc/JAC_PoC_Absorption/Scripts/JAC_Absorption_Gameplay.lua");
    let start = source.find("local function ValidLoyalty(value)").unwrap();
    let end = source
        .find("local function WorkSlots(city, building)")
        .unwrap();
    let code = format!("{}\nTestAdvance=AdvanceLoyalty", &source[start..end]);
    lua.load(&code).exec().unwrap();
    lua
}

#[test]
fn intent_persistence_cannot_leave_the_mutation_on_an_old_city_reference() {
    let lua = fixture(0);
    let completed: bool = lua
        .load("return TestAdvance(Store.Original,Governance,Pending,'immediate',-5)")
        .eval()
        .unwrap();
    assert!(completed);
    let values: (i32, i32, i32) = lua
        .load("return Store.OldCalls,Store.NewCalls,Store.Current.Current")
        .eval()
        .unwrap();
    assert_eq!(values, (0, 1, 95));
    lua.load("TestAdvance(Store.Current,Governance,Pending,'immediate',-5)")
        .exec()
        .unwrap();
    let calls: (i32, i32) = lua
        .load("return Store.OldCalls,Store.NewCalls")
        .eval()
        .unwrap();
    assert_eq!(calls, (0, 1));
}

#[test]
fn changed_owner_after_intent_persistence_blocks_before_native_mutation() {
    let lua = fixture(2);
    assert!(lua
        .load("return TestAdvance(Store.Original,Governance,Pending,'immediate',-5)")
        .eval::<bool>()
        .is_err());
    let calls: (i32, i32) = lua
        .load("return Store.OldCalls,Store.NewCalls")
        .eval()
        .unwrap();
    assert_eq!(calls, (0, 0));
}
