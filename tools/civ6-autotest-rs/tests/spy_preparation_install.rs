#[path = "../src/spy_preparation_install.rs"]
mod installer;
use mlua::Lua;

#[test]
fn generated_module_uses_the_boundary_view_instance_and_gates_before_old_handler() {
    let lua = Lua::new();
    lua.load("ExposedMembers={JAC_PersonaBoundary={ReadEnemy=function() end}}; Game={GetProperty=function() return nil end}; Events={SpyMissionCompleted={Add=function(fn) EVENT=fn end}}; OLD_CALLS=0").exec().unwrap();
    let view = lua.create_table().unwrap();
    let metatable = lua.create_table().unwrap();
    metatable.set("__index", lua.globals()).unwrap();
    view.set_metatable(Some(metatable)).unwrap();
    view.set("_G", view.clone()).unwrap();
    let original = "Events.SpyMissionCompleted.Add(function(attacker, missionID)\nOLD_CALLS=OLD_CALLS+1\nend);";
    let hooked = installer::hooked_view(original).unwrap();
    let script = hooked.replace(
        "include(\"JAC_ReturnedPreparation548\");",
        &installer::connection(),
    );
    // The real Boundary View has local B. The host's InGame global namespace
    // is distinct and must call through the existing UI-only Boundary export.
    view.set(
        "B",
        lua.globals()
            .get::<mlua::Table>("ExposedMembers")
            .unwrap()
            .get::<mlua::Table>("JAC_PersonaBoundary")
            .unwrap(),
    )
    .unwrap();
    lua.load(script)
        .set_environment(view.clone())
        .exec()
        .unwrap();
    lua.load("assert(JAC_ReturnedPreparation548==nil); local p=ExposedMembers.JAC_PersonaBoundary.ReturnedPreparation; assert(type(p.Resume)=='function'); Game.GetProperty=function() return {} end; p.OwnedAuxiliaryEvent=function(a,id) return a==0 and id==3 end").exec().unwrap();
    lua.globals()
        .get::<mlua::Function>("EVENT")
        .unwrap()
        .call::<()>((0, 3))
        .unwrap();
    assert_eq!(view.get::<i64>("OLD_CALLS").unwrap(), 0);
    lua.globals()
        .get::<mlua::Function>("EVENT")
        .unwrap()
        .call::<()>((0, 2))
        .unwrap();
    lua.globals()
        .get::<mlua::Function>("EVENT")
        .unwrap()
        .call::<()>((1, 3))
        .unwrap();
    assert_eq!(view.get::<i64>("OLD_CALLS").unwrap(), 2);
    assert_eq!(installer::hooked_view(&hooked).unwrap(), hooked);
}

#[test]
fn generator_writes_only_owned_files_and_rejects_ambiguous_hook_site() {
    assert!(installer::hooked_view("no event callback").is_err());
    let source = "Events.SpyMissionCompleted.Add(function(attacker, missionID)\nend);\n";
    assert!(installer::hooked_view(&source.repeat(2)).is_err());
    let root = std::env::temp_dir().join(format!("todo548-install-{}", std::process::id()));
    let view = root.join("poc/JAC_Test_PersonaBoundary/UI/JAC_PersonaBoundary_View.lua");
    std::fs::create_dir_all(view.parent().unwrap()).unwrap();
    std::fs::write(&view, source).unwrap();
    installer::write(&root).unwrap();
    let first = std::fs::read(&view).unwrap();
    installer::write(&root).unwrap();
    assert_eq!(std::fs::read(&view).unwrap(), first);
    assert_eq!(
        std::fs::read_to_string(root.join(installer::OUTPUT)).unwrap(),
        installer::connection()
    );
    std::fs::remove_file(root.join(installer::OUTPUT)).unwrap();
    std::fs::remove_file(view).unwrap();
    std::fs::remove_dir(root.join("poc/JAC_Test_PersonaBoundary/UI")).unwrap();
    std::fs::remove_dir(root.join("poc/JAC_Test_PersonaBoundary")).unwrap();
    std::fs::remove_dir(root.join("poc")).unwrap();
    std::fs::remove_dir(root).unwrap();
}
