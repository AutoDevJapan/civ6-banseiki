use mlua::Lua;
use std::{fs, path::PathBuf};

fn load(lua: &Lua, name: &str) -> mlua::Result<()> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../poc/JAC_PoC_Absorption/Scripts")
        .join(name);
    lua.load(fs::read_to_string(path).map_err(mlua::Error::external)?)
        .set_name(name)
        .exec()
}

fn setup() -> mlua::Result<Lua> {
    let lua = Lua::new();
    lua.load(include_str!("fixtures/absorption_world.lua"))
        .call::<()>(lua.create_table()?)?;
    lua.load("function include(_) end").exec()?;
    load(&lua, "JAC_Absorption_Rules.lua")?;
    load(&lua, "JAC_Absorption_Gameplay.lua")?;
    lua.load(
        r#"
        WORK_CITIES={[4]=true}
        CREATOR='Original creator'
        CityManager.TransferCity(CITIES[16],2)
        local cities=Players[2].GetCities
        Players[2].GetCities=function(self)
          local collection=cities(self)
          collection.GetCapitalCity=function() return CITIES[12] end
          return collection
        end
        GameInfo.GreatWorks={[4]={GreatWorkType='GREATWORK_TEST_RELIC',
          GreatWorkObjectType='GREATWORKOBJECT_RELIC',Tourism=8}}
        ExposedMembers.JAC_HeroReadAPI={ReadNativeWork=function()
          return {Type=4,CreatorName=CREATOR,TurnCreated=2,CreatedByPlayer=0}
        end}
        for _,city in pairs(CITIES) do
          local original=city.GetBuildings
          city.GetBuildings=function(self)
            local buildings=original(self)
            buildings.GetGreatWorkInSlot=function(_,building,slot)
              return WORK_CITIES[city:GetX()] and building==1 and slot==0 and 900 or -1
            end
            return buildings
          end
        end
        A=JAC_AbsorptionGameplay or ExposedMembers.JAC_Absorption
        local source=assert(A.OwnedGreatWorkSlots(1)[1])
        local state=A.GetState()
        state.Archive.native={ID='native',Kind='ITEM',Owner=-1,
          Type='GREATWORKOBJECT_RELIC',NativeWork=source}
        Game:SetProperty(A.Rules.STATE_KEY,state)
        ARCHIVE_BEFORE=A.Canonical(A.GetState().Archive)
        function recover() return A.RecoverArchived('native',2,'recovery:native',nil) end
        function assert_retained()
          assert(A.GetRoster(2).native==nil,'native work was awarded without a physical move')
          assert(A.Canonical(A.GetState().Archive)==ARCHIVE_BEFORE,'original archive changed')
          assert(A.GetState().Recoveries['recovery:native'].Status~='COMPLETE')
        end
        "#,
    )
    .exec()?;
    Ok(lua)
}

#[test]
fn recovery_rejects_native_work_still_held_by_another_player() -> mlua::Result<()> {
    let lua = setup()?;
    lua.load("assert(not pcall(recover));assert_retained()")
        .exec()
}

#[test]
fn recovery_rejects_missing_or_duplicated_native_index() -> mlua::Result<()> {
    let lua = setup()?;
    lua.load(
        "WORK_CITIES={};assert(not pcall(recover));assert_retained();\
         WORK_CITIES={[4]=true,[16]=true};assert(not pcall(recover));assert_retained()",
    )
    .exec()
}

#[test]
fn recovery_rejects_changed_native_provenance() -> mlua::Result<()> {
    let lua = setup()?;
    lua.load(
        "WORK_CITIES={[16]=true};CREATOR='Another creator';\
         assert(not pcall(recover));assert_retained()",
    )
    .exec()
}

#[test]
fn recovery_requires_the_original_index_and_records_its_actual_city_once() -> mlua::Result<()> {
    let lua = setup()?;
    lua.load(
        r#"
        WORK_CITIES={[16]=true}
        local recovered=recover()
        assert(recovered.Status=='COMPLETE')
        local item=assert(A.GetRoster(2).native)
        assert(item.NativeWork.Index==900 and item.NativeWork.Owner==2)
        assert(item.City.Owner==2 and item.City.X==16 and item.City.Y==0,
          'work was incorrectly assigned to the capital')
        assert(item.NativeWork.CreatorName=='Original creator'
          and item.NativeWork.TurnCreated==2 and item.NativeWork.CreatedByPlayer==0)
        assert(A.Canonical(A.GetState().Archive)==ARCHIVE_BEFORE)
        ROSTER_BEFORE=A.Canonical(A.GetRoster(2))
        Game.props=A.Rules.Copy(Game.props)
        "#,
    )
    .exec()?;
    load(&lua, "JAC_Absorption_Gameplay.lua")?;
    lua.load(
        "A=JAC_AbsorptionGameplay or ExposedMembers.JAC_Absorption;\
         assert(recover().Status=='COMPLETE');\
         assert(A.Canonical(A.GetRoster(2))==ROSTER_BEFORE);\
         assert(A.Canonical(A.GetState().Archive)==ARCHIVE_BEFORE)",
    )
    .exec()
}
