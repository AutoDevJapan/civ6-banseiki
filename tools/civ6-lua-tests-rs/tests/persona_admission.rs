//! The private cost-one admission must be derived by the shipped Gameplay Lua.
use civ6_lua_tests_rs::{new_lua, read_repo};
use mlua::{Function, Lua, Table};

const GAMEPLAY: &str = "poc/JAC_PoC_LeaderPersona/Scripts/JAC_LeaderPersona_Gameplay.lua";
const RULES: &str = "poc/JAC_PoC_LeaderPersona/Shared/JAC_PersonaTreeRules.lua";

fn run_case(source: &str, name: &str) -> mlua::Result<()> {
    let lua = new_lua();
    load(
        &lua,
        include_str!("../fixtures/persona/stubs.lua"),
        "stubs.lua",
    )?;
    load(
        &lua,
        include_str!("../fixtures/persona/admission_engine.lua"),
        "admission_engine.lua",
    )?;
    load(&lua, &read_repo(RULES), "JAC_PersonaTreeRules.lua")?;
    load(&lua, source, "JAC_LeaderPersona_Gameplay.lua")?;
    load(
        &lua,
        include_str!("../fixtures/persona/admission_cases.lua"),
        "admission_cases.lua",
    )?;
    let cases: Table = lua.globals().get("ADMISSION_CASES")?;
    let case: Function = cases.get(name)?;
    case.call(())
}

fn load(lua: &Lua, source: &str, name: &str) -> mlua::Result<()> {
    lua.load(source).set_name(name).exec()
}

fn shipped_case(name: &str) {
    run_case(&read_repo(GAMEPLAY), name).unwrap_or_else(|error| panic!("{name}: {error}"));
}

macro_rules! admission_tests {
    ($($name:ident),+ $(,)?) => {$(
        #[test]
        fn $name() { shipped_case(stringify!($name)); }
    )+};
}

admission_tests!(
    join_prepare_native_appoint_finalize,
    missing_person_stays_closed,
    duplicate_person_stays_closed,
    other_reservation_stays_closed,
    wrong_owner_never_opens_owner_zero,
    inactive_flag_never_publishes,
    wrong_governor_index_never_publishes,
    spent_title_never_counts_as_available,
    reconcile_revokes_after_person_or_owner_loss,
);

#[test]
fn disabling_shipped_admission_sync_breaks_the_positive_path() {
    let shipped = read_repo(GAMEPLAY);
    let opening = "local allowed = false;";
    assert_eq!(
        shipped.matches(opening).count(),
        1,
        "admission opening moved"
    );
    let disabled = shipped.replacen(
        opening,
        "local allowed = false; if true then return; end",
        1,
    );
    let error = run_case(&disabled, "join_prepare_native_appoint_finalize")
        .expect_err("positive path must reject a disabled SyncAshinaAdmission");
    assert!(
        error
            .to_string()
            .contains("preparation did not publish Ashina admission"),
        "mutation failed at an unrelated assertion: {error}"
    );
}
