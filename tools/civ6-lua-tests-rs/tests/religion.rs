use civ6_lua_tests_rs::strict::{assert_python_truth, truthy_results, ChunkExt, Data};
use civ6_lua_tests_rs::{json_to_lua, new_lua, query_json, read_repo, repo_root};
use mlua::{Function, Lua, MultiValue, Value};
use rusqlite::Connection;
use serde_json::{json, Map, Value as Json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

const R1: &str = "JAC_RP_PLACEHOLDER_01";
const R2: &str = "JAC_RP_PLACEHOLDER_02";
const KEY: &str = "JAC_RP_STATE_V1";
const MOD: &str = "poc/JAC_PoC_ReligionPersistence";
const TABLES: [&str; 6] = [
    "Religions",
    "AssetKinds",
    "Jobs",
    "Sites",
    "Outcomes",
    "Rules",
];

macro_rules! call { ($h:expr, $name:literal $(, $arg:expr)* $(,)?) => { $h.call($name, vec![$(json!($arg)),*]) }; }

fn install_absorption_adapter(h: &Harness) {
    let globals = h.lua.globals();
    globals
        .set(
            "RP_SOURCE",
            read_repo(format!("{MOD}/Scripts/JAC_ReligionPersistence.lua")),
        )
        .unwrap();
    globals
        .set(
            "RP_ADAPTER_SOURCE",
            read_repo(format!("{MOD}/Scripts/JAC_RP_AbsorptionAdapter.lua")),
        )
        .unwrap();
    globals
        .set(
            "ABS_ENTRY_SOURCE",
            read_repo("poc/JAC_PoC_Absorption/Scripts/JAC_Absorption_Entry.lua"),
        )
        .unwrap();
    h.exec(
        r#"
        JAC_ReligionPersistenceGameplay=nil
        JAC_AbsorptionGameplay={Rules={Copy=function(value) return value end},
          RegisterAdapter=function(name, value)
            assert(name=='religion-persistence' and ADAPTER==nil)
            ADAPTER=value
          end}
        JAC_AbsorptionFlowGameplay={}
        GameEvents.JAC_Absorption_TestCall={Add=function() end}
        GameConfiguration.IsAnyMultiplayer=function() return false end
        include=function(name)
          if name=='JAC_Absorption_Gameplay' or name=='JAC_Absorption_Flow' then return end
          local source=name=='JAC_ReligionPersistence' and RP_SOURCE
            or name=='JAC_RP_AbsorptionAdapter' and RP_ADAPTER_SOURCE
          assert(source, 'unexpected VFS include: '..name)
          assert(loadstring(source))()
        end
        assert(loadstring(ABS_ENTRY_SOURCE))()
        assert(ADAPTER and JAC_ReligionPersistenceGameplay)
        "#,
    );
}

fn absorption_tx(h: &Harness, transfer: bool) {
    h.lua
        .globals()
        .set(
            "TX",
            json_to_lua(
                &h.lua,
                &json!({
                    "ID":"rp-absorb-1", "From":0, "To":1,
                    "Units":[{"ID":77,"Type":"UNIT_HERO_ANANSI","Hero":{"Class":10},"KeepUnit":transfer,
                        "Destination":1,"Outcome":if transfer {"RECRUIT"} else {"SUICIDE"}}],
                    "UnitMap":{"0:77":{"ID":88,"Token":"rp-absorb-1:unit:77"}}
                }),
            ),
        )
        .unwrap();
}

fn seeded_absorption_harness() -> Harness {
    let h = Harness::new();
    h.f();
    call!(h, "BindEngineReligion", R1, 42);
    call!(h, "SetCommunity", R1, "origin", 4, 2, "PLAYER_0");
    for (id, kind, holder) in [
        ("scripture", "SCRIPTURE", "CAVE"),
        ("record", "RECORD", "PLAYER_0"),
        ("hidden", "SCRIPTURE", "HIDDEN_PLAYER_0"),
        ("vault", "RELIC", "VAULT_PLAYER_0"),
        ("hero_source", "RELIC", "HERO_PLAYER_0"),
        ("unresolved_source", "SCRIPTURE", "PLAYER_0"),
    ] {
        call!(h, "PreserveAsset", R1, id, kind, holder, 0);
    }
    h.found(R2, 2, 1, 2);
    call!(h, "SetCommunity", R2, "origin", 2, 1, "PLAYER_2");
    call!(h, "PreserveAsset", R2, "third", "RECORD", "PLAYER_2", 2);
    h.lua.globals().set("RP_R1", R1).unwrap();
    h.lua.globals().set("RP_R2", R2).unwrap();
    h.exec(
        r#"
        local s=PROPERTY_STORE.JAC_RP_STATE_V1
        local r=s.religions[RP_R1]
        r.discoveries.history={asset='hero_source',player=0,outcome='HERO',effects='APPLIED'}
        r.materialUses.hero_source={outcome='HERO',player=0,effects='APPLIED'}
        r.discoveries.unresolved={asset='unresolved_source',player=0,
          actor='PLAYER_0',site='CAVE',job='MONK'}
        r.revivals.old={source='scripture',community='past',holder='PLAYER_0',
          player=0,effects='APPLIED',turn=2}
        r.revivalCount=1
        r.outcomeUses['0:HERO']='history'
        r.retiredAssets.retired=true
        s.heroes['0']={religion=RP_R1,discovery='history',unit=77,
          heroClass=10,heroUnit='UNIT_HERO_ANANSI',verification='APPLIED'}
        s.artifacts['0']={religion=RP_R1,asset='vault'}
        "#,
    );
    install_absorption_adapter(&h);
    absorption_tx(&h, true);
    h
}

#[test]
fn absorption_entry_moves_rp_ledger_and_re_registers_after_load() {
    let mut h = seeded_absorption_harness();
    let third = h.eval("PROPERTY_STORE.JAC_RP_STATE_V1.religions[RP_R2]");
    let before = h.s();
    let writes = h.writes();
    let snapshot = h.eval("ADAPTER.Prepare(TX)");
    assert_eq!(h.writes(), writes, "Prepare must only read");
    h.lua
        .globals()
        .set("SNAP", snapshot.to_lua(&h.lua))
        .unwrap();
    call!(h, "SetCommunity", R1, "late", 1, 1, "PLAYER_0");
    call!(h, "SetCommunity", R2, "late-third", 1, 0, "PLAYER_2");
    let third_at_apply = h.eval("PROPERTY_STORE.JAC_RP_STATE_V1.religions[RP_R2]");
    let writes = h.writes().python_f64().unwrap();
    h.exec("ADAPTER.Apply(TX,SNAP)");
    assert_eq!(h.writes().python_f64().unwrap(), writes + 1.0);
    h.exec("ADAPTER.Verify(TX,SNAP)");
    h.exec("ADAPTER.Apply(TX,SNAP)");
    assert_eq!(
        h.writes().python_f64().unwrap(),
        writes + 1.0,
        "retry must not write twice"
    );
    let after = h.s();
    assert_eq!(after["founder"], before["founder"]);
    assert_eq!(after["founderDefeated"], true);
    assert_eq!(after["engineReligion"], before["engineReligion"]);
    assert_eq!(after["communities"]["origin"]["followers"], 4);
    assert_eq!(after["communities"]["origin"]["publicFacilities"], 2);
    assert_eq!(after["communities"]["origin"]["holder"], "PLAYER_1");
    assert_eq!(after["communities"]["late"]["holder"], "PLAYER_1");
    assert_eq!(after["assets"]["scripture"]["holder"], "CAVE");
    assert_eq!(after["assets"]["scripture"]["owner"], 1);
    for (asset, holder) in [
        ("record", "PLAYER_1"),
        ("hidden", "HIDDEN_PLAYER_1"),
        ("vault", "VAULT_PLAYER_1"),
        ("hero_source", "HERO_PLAYER_1"),
    ] {
        assert_eq!(after["assets"][asset]["holder"], holder);
    }
    assert_eq!(after["discoveries"]["history"]["player"], 0);
    assert_eq!(after["discoveries"]["unresolved"]["player"], 1);
    assert_eq!(after["discoveries"]["unresolved"]["actor"], "PLAYER_1");
    assert_eq!(after["materialUses"]["hero_source"]["player"], 0);
    assert_eq!(after["revivals"]["old"]["player"], 0);
    assert_eq!(after["revivals"]["old"]["holder"], "PLAYER_1");
    assert_eq!(after["outcomeUses"]["0:HERO"], "history");
    assert_eq!(after["retiredAssets"]["retired"], true);
    assert_eq!(
        h.eval("PROPERTY_STORE.JAC_RP_STATE_V1.heroes['1'].unit"),
        88
    );
    assert_eq!(
        h.eval("PROPERTY_STORE.JAC_RP_STATE_V1.artifacts['1'].asset"),
        "vault"
    );
    assert_eq!(
        h.eval("PROPERTY_STORE.JAC_RP_STATE_V1.religions[RP_R2]"),
        third_at_apply
    );
    assert_ne!(
        third, third_at_apply,
        "the intervening RP update must be visible"
    );
    h.reload();
    install_absorption_adapter(&h);
    absorption_tx(&h, true);
    h.lua
        .globals()
        .set("SNAP", snapshot.to_lua(&h.lua))
        .unwrap();
    h.exec("ADAPTER.Verify(TX,SNAP); ADAPTER.Apply(TX,SNAP)");
    assert_eq!(h.writes(), 0);
    h.exec("TURN=TURN+1; ENGINE['0'].cities['0'].pressure={['43']=7168}");
    h.exec("ADAPTER.Verify(TX,SNAP)");
}

#[test]
fn absorption_adapter_rejects_pending_conflicts_and_nontransferred_hero_before_writing() {
    for (change, label) in [
        (
            "s.religions[RP_R1].discoveries.history.effects='PENDING'",
            "discovery",
        ),
        (
            "s.religions[RP_R1].materialUses.hero_source.effects='PENDING'",
            "material",
        ),
        (
            "s.religions[RP_R1].revivals.old.effects='PENDING'",
            "revival",
        ),
        ("s.heroes['0'].verification='PENDING'", "hero"),
        ("s.revision='broken'", "revision type"),
        ("s.revision=-1", "negative revision"),
        ("s.revision=1.5", "fractional revision"),
        ("s.revision=2147483648", "revision overflow"),
        ("s.absorptionReceipts=false", "receipt type"),
        (
            "s.absorptionReceipts={['rp-absorb-1']={}}",
            "receipt ID collision",
        ),
        (
            "s.heroes['0'].heroClass=11",
            "reused hero ID with another class",
        ),
        (
            "s.heroes['0'].heroUnit='UNIT_HERO_ARTHUR'",
            "hero type mismatch",
        ),
        ("s.heroes['1']={unit=99}", "hero collision"),
        ("s.artifacts['1']={asset='other'}", "Vault collision"),
    ] {
        let h = seeded_absorption_harness();
        h.exec(&format!("local s=PROPERTY_STORE.JAC_RP_STATE_V1;{change}"));
        let before = h.export();
        let writes = h.writes();
        let error = h
            .lua
            .load("return ADAPTER.Prepare(TX)")
            .eval::<MultiValue>()
            .unwrap_err();
        assert!(!error.to_string().is_empty(), "{label}");
        assert_eq!(h.export(), before, "{label}");
        assert_eq!(h.writes(), writes, "{label}");
    }
    let h = seeded_absorption_harness();
    absorption_tx(&h, false);
    assert!(h
        .lua
        .load("return ADAPTER.Prepare(TX)")
        .eval::<MultiValue>()
        .is_err());
}

#[test]
fn absorption_receipt_does_not_freeze_unrelated_destination_rewards() {
    let mut h = Harness::new();
    h.f();
    call!(h, "PreserveAsset", R1, "source", "SCRIPTURE", "CAVE", 0);
    call!(h, "Found", R2, 2, "origin", 1, 0, "PLAYER_1");
    call!(h, "SetCommunity", R2, "origin", 0, 0, "PLAYER_1");
    h.discovery("MONK", "MONK_1", R2, "book2", "d2", 1);
    h.resolve(R2, "d2", "HERO", json!({"player":1}));
    assert_eq!(hero_verification(&h), "PENDING");
    h.exec("PROPERTY_STORE.JAC_RP_STATE_V1.artifacts['1']={religion='JAC_RP_PLACEHOLDER_02',asset='vault_book'}");
    let hero_before = h.export()["heroes"]["1"].clone();
    let vault_before = h.export()["artifacts"]["1"].clone();
    install_absorption_adapter(&h);
    h.exec("TX={ID='unrelated-rewards',From=0,To=1,Units={},UnitMap={}}");
    let snapshot = h.eval("ADAPTER.Prepare(TX)");
    h.lua
        .globals()
        .set("SNAP", snapshot.to_lua(&h.lua))
        .unwrap();
    h.exec("ADAPTER.Apply(TX,SNAP); ADAPTER.Verify(TX,SNAP)");
    assert_eq!(h.export()["heroes"]["1"], hero_before);
    assert_eq!(h.export()["artifacts"]["1"], vault_before);
    assert_eq!(h.eval("PROPERTY_STORE.JAC_RP_STATE_V1.absorptionReceipts[TX.ID].Hero==nil and PROPERTY_STORE.JAC_RP_STATE_V1.absorptionReceipts[TX.ID].Artifact==nil"), true);
    h.exec("HERO_UI_TICK()");
    assert_eq!(hero_verification(&h), "APPLIED");
    let saved = h.export();
    h.exec("ADAPTER.Verify(TX,SNAP); ADAPTER.Apply(TX,SNAP)");
    assert_eq!(h.export(), saved);
    h.reload();
    install_absorption_adapter(&h);
    h.exec("TX={ID='unrelated-rewards',From=0,To=1,Units={},UnitMap={}}");
    h.lua
        .globals()
        .set("SNAP", snapshot.to_lua(&h.lua))
        .unwrap();
    h.exec("ADAPTER.Verify(TX,SNAP); ADAPTER.Apply(TX,SNAP)");
    assert_eq!(h.export(), saved);
    assert_eq!(h.writes(), 0);
}

#[test]
fn absorption_adapter_saturated_counter_allows_normal_updates_and_reload() {
    let mut h = seeded_absorption_harness();
    h.exec("PROPERTY_STORE.JAC_RP_STATE_V1.revision=2147483647");
    let snapshot = h.eval("ADAPTER.Prepare(TX)");
    h.lua
        .globals()
        .set("SNAP", snapshot.to_lua(&h.lua))
        .unwrap();
    h.exec("ADAPTER.Apply(TX,SNAP); ADAPTER.Verify(TX,SNAP)");
    assert_eq!(h.export()["revision"], 2147483647);
    assert_eq!(h.export()["absorptionExactReceipt"], "rp-absorb-1");
    h.exec("local a=PROPERTY_STORE.JAC_RP_STATE_V1.religions[RP_R1].assets.scripture; a.owner=9; assert(not pcall(ADAPTER.Verify,TX,SNAP)); a.owner=1");
    call!(h, "SetCommunity", R1, "late_max", 0, 0, "PLAYER_1");
    assert_eq!(h.export()["revision"], 2147483647);
    assert_eq!(
        h.eval("PROPERTY_STORE.JAC_RP_STATE_V1.absorptionExactReceipt==nil"),
        true
    );
    let saved = h.export();
    let writes = h.writes();
    h.exec("ADAPTER.Verify(TX,SNAP); ADAPTER.Apply(TX,SNAP)");
    assert_eq!(h.export(), saved);
    assert_eq!(h.writes(), writes);
    h.reload();
    install_absorption_adapter(&h);
    absorption_tx(&h, true);
    h.lua
        .globals()
        .set("SNAP", snapshot.to_lua(&h.lua))
        .unwrap();
    h.exec("ADAPTER.Verify(TX,SNAP); ADAPTER.Apply(TX,SNAP)");
    assert_eq!(h.export(), saved);
    assert_eq!(h.writes(), 0);
    // V1 receipts remain readable after an ordinary update at the saturated counter.
    h.exec("PROPERTY_STORE.JAC_RP_STATE_V1.absorptionReceipts[TX.ID].Version=1");
    h.exec("ADAPTER.Verify(TX,SNAP); ADAPTER.Apply(TX,SNAP)");
    assert_eq!(h.writes(), 0);
}

#[test]
fn absorption_adapter_failed_write_retries_and_empty_state_is_supported() {
    let h = seeded_absorption_harness();
    let snapshot = h.eval("ADAPTER.Prepare(TX)");
    h.lua
        .globals()
        .set("SNAP", snapshot.to_lua(&h.lua))
        .unwrap();
    let before = h.export();
    let writes = h.writes().python_f64().unwrap();
    h.exec("FAIL_WRITE=true");
    assert!(h.lua.load("ADAPTER.Apply(TX,SNAP)").exec().is_err());
    assert_eq!(h.export(), before);
    assert_eq!(h.writes().python_f64().unwrap(), writes);
    h.exec("FAIL_WRITE=false; ADAPTER.Apply(TX,SNAP); ADAPTER.Verify(TX,SNAP)");
    assert_eq!(h.writes().python_f64().unwrap(), writes + 1.0);

    let mut empty = Harness::new();
    install_absorption_adapter(&empty);
    empty.exec("TX={ID='empty',From=0,To=1,Units={},UnitMap={}}");
    let snapshot = empty.eval("ADAPTER.Prepare(TX)");
    empty
        .lua
        .globals()
        .set("SNAP", snapshot.to_lua(&empty.lua))
        .unwrap();
    empty.exec("ADAPTER.Apply(TX,SNAP); ADAPTER.Verify(TX,SNAP)");
    assert_eq!(empty.writes(), 1);
    assert_eq!(
        empty.eval("PROPERTY_STORE.JAC_RP_STATE_V1.absorptionReceipts.empty.Revision"),
        1
    );
    empty.f();
    let after_found = empty.export();
    let writes = empty.writes();
    empty.exec("ADAPTER.Verify(TX,SNAP); ADAPTER.Apply(TX,SNAP)");
    assert_eq!(empty.export(), after_found);
    assert_eq!(empty.writes(), writes);
    empty.reload();
    install_absorption_adapter(&empty);
    empty.exec("TX={ID='empty',From=0,To=1,Units={},UnitMap={}}");
    empty
        .lua
        .globals()
        .set("SNAP", snapshot.to_lua(&empty.lua))
        .unwrap();
    empty.exec("ADAPTER.Verify(TX,SNAP); ADAPTER.Apply(TX,SNAP)");
    assert_eq!(empty.export(), after_found);
    assert_eq!(empty.writes(), 0);
    empty.exec("PROPERTY_STORE.JAC_RP_STATE_V1.absorptionReceipts.empty=nil");
    assert!(empty.lua.load("ADAPTER.Verify(TX,SNAP)").exec().is_err());
}

#[test]
fn absorption_adapter_keeps_native_revival_available_to_new_owner() {
    let mut h = Harness::new();
    h.f();
    call!(h, "PreserveAsset", R1, "source", "SCRIPTURE", "CAVE", 0);
    call!(h, "BindEngineReligion", R1, 42);
    h.exec("ENGINE['0'].cities['0'].pressure={['43']=7168}; ENGINE['1'].cities['0'].pressure={['43']=4096}");
    install_absorption_adapter(&h);
    h.exec("TX={ID='revival-transfer',From=0,To=1,Units={},UnitMap={}}");
    let snapshot = h.eval("ADAPTER.Prepare(TX)");
    h.lua
        .globals()
        .set("SNAP", snapshot.to_lua(&h.lua))
        .unwrap();
    h.exec("ADAPTER.Apply(TX,SNAP); ADAPTER.Verify(TX,SNAP)");
    assert_eq!(h.s()["assets"]["source"]["owner"], 1);
    assert_eq!(call!(h, "NativeFollowers", R1), 0);
    let faith = h.engine("1")["faith"].python_f64().unwrap();
    call!(h, "Revive", R1, "after-absorption", "source", "new", 1, 0);
    assert_eq!(call!(h, "NativeFollowers", R1), 1);
    assert_eq!(h.engine("1")["faith"].python_f64().unwrap(), faith - 20.0);
    h.exec("ADAPTER.Verify(TX,SNAP)");
    h.reload();
    install_absorption_adapter(&h);
    h.exec("TX={ID='revival-transfer',From=0,To=1,Units={},UnitMap={}}");
    h.lua
        .globals()
        .set("SNAP", snapshot.to_lua(&h.lua))
        .unwrap();
    h.exec("ADAPTER.Verify(TX,SNAP)");
    assert_eq!(h.s()["revivals"]["after-absorption"]["effects"], "APPLIED");
    assert_eq!(call!(h, "NativeFollowers", R1), 1);
}

#[test]
fn rejection_requires_a_raw_lua_error_and_independently_decodable_unchanged_state() {
    use std::panic::{catch_unwind, AssertUnwindSafe};
    for returned in [
        "function() end",
        "(function() local t={};t.self=t;return t end)()",
        "nil",
        "false",
    ] {
        let h = Harness::new();
        h.exec(&format!("rp.Probe=function() return {returned} end"));
        assert!(h.raw_call("Probe", vec![]).is_ok());
        assert!(
            catch_unwind(AssertUnwindSafe(|| h.rejected("Probe", vec![]))).is_err(),
            "{returned}"
        );
    }
    let h = Harness::new();
    h.exec("rp.Probe=function() error('intentional refusal') end");
    h.rejected("Probe", vec![]);
    h.exec("rp.Probe=function() rp.Snapshot=function() return function() end end; error('intentional refusal') end");
    assert!(catch_unwind(AssertUnwindSafe(|| h.rejected("Probe", vec![]))).is_err());
}

#[test]
fn founder_truth_and_recursive_research_equality_accept_numeric_booleans() {
    let h = Harness::new();
    h.f();
    h.exec("local inspect=rp.Inspect; rp.Inspect=function(...) local s=inspect(...);s.founderDefeated=1;return s end; ENGINE['1'].boosted={['2']=1,['4']=1}");
    assert_python_truth(&h.s()["founderDefeated"], true);
    assert_eq!(h.engine("1")["boosted"], json!({"2":true,"4":true}));
    h.exec("rp.CanPersecute=function() return {} end; rp.CompleteHero=function() return {} end");
    assert!(h.call_truth("CanPersecute", vec![]));
    assert!(h.call_truth("CompleteHero", vec![]));
}

#[test]
fn export_comparison_detects_numeric_key_changes_and_restore_keeps_both_keys() {
    let mut h = Harness::new();
    h.f();
    h.reload();
    h.exec("local export=rp.Export; rp.Export=function(...) local data=export(...); data.probe={[0]='same',['0']='same'};return data end");
    let before = h.export();
    assert_eq!(before["probe"].entries().len(), 2);
    let fresh = new_lua();
    fresh
        .globals()
        .set("RESTORED", before.to_lua(&fresh))
        .unwrap();
    assert_eq!(
        fresh.load("return RESTORED").strict_eval::<Data>().unwrap(),
        before
    );
    fresh.load("RESTORED.probe[0]=nil").exec().unwrap();
    assert_ne!(
        fresh.load("return RESTORED").strict_eval::<Data>().unwrap(),
        before
    );
    fresh.load("RESTORED.probe={}").exec().unwrap();
    let empty = fresh.load("return RESTORED").strict_eval::<Data>().unwrap();
    fresh.load("RESTORED.probe=nil").exec().unwrap();
    let absent = fresh.load("return RESTORED").strict_eval::<Data>().unwrap();
    assert_ne!(empty, absent);
    assert_ne!(absent["probe"], Data::Nil);
}

fn catalog() -> Json {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE Modifiers (ModifierId TEXT PRIMARY KEY, ModifierType TEXT, RunOnce INT, Permanent INT);\
        CREATE TABLE ModifierArguments (ModifierId TEXT, Name TEXT, Value TEXT, PRIMARY KEY(ModifierId, Name));").unwrap();
    db.execute_batch(&read_repo(format!(
        "{MOD}/Data/JAC_ReligionPersistence.sql"
    )))
    .unwrap();
    let mut result = Map::new();
    for name in TABLES {
        result.insert(
            name.into(),
            Json::Array(query_json(&db, &format!("SELECT * FROM JAC_RP_{name}"))),
        );
    }
    let modifiers = query_json(&db, "SELECT * FROM Modifiers");
    let mut args = Map::new();
    let mut statement = db
        .prepare("SELECT Name, Value FROM ModifierArguments WHERE ModifierId=?")
        .unwrap();
    for modifier in &modifiers {
        let id = modifier["ModifierId"].as_str().unwrap();
        let rows = statement
            .query_map([id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .unwrap();
        args.insert(
            id.into(),
            Json::Object(
                rows.map(|r| {
                    let (key, val) = r.unwrap();
                    (key, Json::String(val))
                })
                .collect(),
            ),
        );
    }
    result.insert("modifiers".into(), Json::Object(args));
    result.insert("modifierTypes".into(), Json::Array(modifiers));
    Json::Object(result)
}

struct Harness {
    lua: Lua,
    tables: Json,
}

impl Harness {
    fn new() -> Self {
        let mut result = Self {
            lua: new_lua(),
            tables: catalog(),
        };
        result.load(None, None, None).unwrap();
        result
    }

    fn load(
        &mut self,
        saved: Option<Data>,
        tables: Option<Json>,
        engine: Option<Data>,
    ) -> mlua::Result<()> {
        self.lua = new_lua();
        let data = tables.unwrap_or_else(|| self.tables.clone());
        let table_data = Json::Object(
            TABLES
                .iter()
                .map(|name| (name.to_string(), data[*name].clone()))
                .collect(),
        );
        let args = (
            json_to_lua(&self.lua, &table_data),
            saved
                .as_ref()
                .map(|x| x.to_lua(&self.lua))
                .unwrap_or(Value::Nil),
            engine
                .as_ref()
                .map(|x| x.to_lua(&self.lua))
                .unwrap_or(Value::Nil),
            json_to_lua(&self.lua, &data["modifiers"]),
        );
        self.lua
            .load(read_repo(
                "tools/civ6-lua-tests-rs/fixtures/religion/stubs.lua",
            ))
            .call::<()>(args)?;
        self.lua
            .load(read_repo(format!(
                "{MOD}/Scripts/JAC_ReligionPersistence.lua"
            )))
            .exec()?;
        self.lua
            .load(read_repo(format!(
                "{MOD}/UI/JAC_ReligionPersistence_HeroUI.lua"
            )))
            .exec()?;
        let rp: mlua::Table = self
            .lua
            .globals()
            .get::<mlua::Table>("ExposedMembers")?
            .get("JAC_ReligionPersistence")?;
        self.lua.globals().set("rp", rp)?;
        // These fixtures start during a human turn. Drive the real Gameplay
        // hook, rather than letting the UI's IsTurnActive authorize effects.
        self.lua
            .load("RELIGION_TURN_START(0); RELIGION_TURN_START(1); WRITES=0")
            .exec()?;
        Ok(())
    }

    fn function(&self, name: &str) -> Function {
        self.lua
            .globals()
            .get::<mlua::Table>("rp")
            .unwrap()
            .get(name)
            .unwrap()
    }
    fn raw_call(&self, name: &str, args: Vec<Json>) -> mlua::Result<MultiValue> {
        let values = args.iter().map(|x| json_to_lua(&self.lua, x)).collect();
        self.function(name)
            .call::<MultiValue>(MultiValue::from_vec(values))
    }
    fn call(&self, name: &str, args: Vec<Json>) -> Data {
        Data::from_lupa_results(
            self.raw_call(name, args)
                .unwrap_or_else(|e| panic!("{name}: {e}")),
        )
        .unwrap_or_else(|e| panic!("{name}: {e}"))
    }
    fn call_truth(&self, name: &str, args: Vec<Json>) -> bool {
        truthy_results(self.raw_call(name, args).unwrap())
    }
    fn truth(&self, expr: &str) -> bool {
        self.lua
            .load(format!("return {expr}"))
            .python_truthy()
            .unwrap()
    }
    fn exec(&self, code: &str) {
        self.lua
            .load(code)
            .exec()
            .unwrap_or_else(|e| panic!("{code}: {e}"));
    }
    fn eval(&self, expr: &str) -> Data {
        self.lua
            .load(format!("return {expr}"))
            .strict_eval()
            .unwrap_or_else(|e| panic!("{expr}: {e}"))
    }
    fn eval_chunk(&self, code: &str) -> Data {
        self.lua
            .load(code)
            .strict_eval()
            .unwrap_or_else(|e| panic!("{code}: {e}"))
    }
    fn state(&self, religion: &str) -> Data {
        self.call("Inspect", vec![json!(religion)])
    }
    fn s(&self) -> Data {
        self.state(R1)
    }
    fn export(&self) -> Data {
        self.call("Export", vec![])
    }
    fn snapshot(&self) -> Data {
        self.call("Snapshot", vec![])
    }
    fn writes(&self) -> Data {
        self.eval("WRITES")
    }
    fn engine(&self, player: &str) -> Data {
        self.eval(&format!("ENGINE['{player}']"))
    }
    fn found(&self, religion: &str, followers: i64, facilities: i64, founder: i64) {
        self.call(
            "Found",
            vec![
                json!(religion),
                json!(founder),
                json!("origin"),
                json!(followers),
                json!(facilities),
                json!("PLAYER_0"),
            ],
        );
    }
    fn f(&self) {
        self.found(R1, 3, 1, 0);
    }
    fn lost(&self, religion: &str) {
        self.call(
            "SetCommunity",
            vec![
                json!(religion),
                json!("origin"),
                json!(0),
                json!(0),
                json!("PLAYER_0"),
            ],
        );
    }
    fn l(&self) {
        self.lost(R1);
    }
    fn discovery(
        &self,
        job: &str,
        actor: &str,
        religion: &str,
        item: &str,
        discovery: &str,
        player: i64,
    ) {
        self.call(
            "Discover",
            vec![
                json!(religion),
                json!(discovery),
                json!(item),
                json!("SCRIPTURE"),
                json!(actor),
                json!(job),
                json!("CAVE"),
                json!(player),
            ],
        );
    }
    fn d(&self) {
        self.discovery("MONK", "MONK_1", R1, "book", "d", 1);
    }
    fn options(&self, extra: Json) -> Json {
        let mut base =
            json!({"city":0,"revival":"r","community":"new","targetReligion":R2,"targetPlayer":0});
        for (k, v) in extra.as_object().unwrap() {
            base[k] = v.clone();
        }
        base
    }
    fn resolve(&self, religion: &str, discovery: &str, outcome: &str, extra: Json) -> Data {
        self.call(
            "ResolveDiscovery",
            vec![
                json!(religion),
                json!(discovery),
                json!(outcome),
                self.options(extra),
            ],
        )
    }
    fn bind_absorption(&self) {
        self.call("BindEngineReligion", vec![json!(R1), json!(42)]);
        self.call("BindEngineReligion", vec![json!(R2), json!(43)]);
    }
    fn reload(&mut self) {
        let snapshot = self.snapshot();
        let saved = self.export();
        let engine = self.eval("ENGINE");
        let turn = self.eval("TURN");
        let active = self.eval("PROPERTY_STORE.JAC_RP_ACTIVE_TURNS_V1");
        self.load(Some(saved), None, Some(engine)).unwrap();
        self.exec(&format!("TURN={turn}"));
        self.lua
            .globals()
            .set("SAVED_ACTIVE", active.to_lua(&self.lua))
            .unwrap();
        self.exec("PROPERTY_STORE.JAC_RP_ACTIVE_TURNS_V1=SAVED_ACTIVE");
        assert_eq!(self.writes(), json!(0));
        assert_eq!(self.snapshot(), snapshot);
    }
    fn rejected(&self, name: &str, args: Vec<Json>) {
        let before = self.snapshot();
        let writes = self.writes();
        assert!(
            self.raw_call(name, args).is_err(),
            "{name} unexpectedly succeeded"
        );
        assert_eq!(self.snapshot(), before, "{name} changed saved state");
        assert_eq!(self.writes(), writes, "{name} wrote state");
    }
}

macro_rules! reject { ($h:expr, $name:literal $(, $arg:expr)* $(,)?) => { $h.rejected($name, vec![$(json!($arg)),*]) }; }

fn manifest_text() -> String {
    read_repo(format!("{MOD}/JAC_PoC_ReligionPersistence.modinfo"))
}
fn child<'a, 'input>(node: roxmltree::Node<'a, 'input>, name: &str) -> roxmltree::Node<'a, 'input> {
    node.children()
        .find(|n| n.is_element() && n.tag_name().name() == name)
        .unwrap_or_else(|| panic!("missing {name}"))
}
fn path_node<'a, 'input>(
    mut node: roxmltree::Node<'a, 'input>,
    path: &str,
) -> roxmltree::Node<'a, 'input> {
    for name in path.split('/') {
        node = child(node, name);
    }
    node
}
fn files_rec(path: &Path, output: &mut BTreeSet<String>) {
    for item in std::fs::read_dir(path).unwrap() {
        let path = item.unwrap().path();
        if path.is_dir() {
            files_rec(&path, output);
        } else if path.extension().and_then(|x| x.to_str()) != Some("modinfo") {
            output.insert(
                path.strip_prefix(repo_root().join(MOD))
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/"),
            );
        }
    }
}

#[test]
fn test_catalog_has_twenty_placeholder_slots_and_ssot_sources() {
    let t = catalog();
    assert_eq!(
        t["Religions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| r["ID"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>(),
        (1..=20)
            .map(|n| format!("JAC_RP_PLACEHOLDER_{n:02}"))
            .collect::<Vec<_>>()
    );
    let kinds: BTreeSet<_> = t["AssetKinds"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["ID"].as_str().unwrap())
        .collect();
    assert_eq!(
        kinds,
        BTreeSet::from(["SCRIPTURE", "RELIC", "RECORD", "PERSON", "HOLY_SITE"])
    );
    let sources: BTreeSet<_> = t["AssetKinds"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["RevivalSource"] == 1)
        .map(|r| r["ID"].as_str().unwrap())
        .collect();
    assert_eq!(
        sources,
        BTreeSet::from(["SCRIPTURE", "RELIC", "RECORD", "PERSON"])
    );
    assert_eq!(
        [
            t["Jobs"].as_array().unwrap().len(),
            t["Sites"].as_array().unwrap().len(),
            t["Outcomes"].as_array().unwrap().len()
        ],
        [7, 5, 9]
    );
}

#[test]
fn test_manifest_packages_every_file_and_separates_gameplay() {
    let text = manifest_text();
    let doc = roxmltree::Document::parse(&text).unwrap();
    let root = doc.root_element();
    let files: BTreeSet<_> = path_node(root, "Files")
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "File")
        .map(|n| n.text().unwrap().to_owned())
        .collect();
    let mut actual = BTreeSet::new();
    files_rec(&repo_root().join(MOD), &mut actual);
    assert_eq!(files, actual);
    assert_eq!(
        path_node(root, "InGameActions/AddGameplayScripts/File").text(),
        Some("Scripts/JAC_ReligionPersistence.lua")
    );
    let ui = path_node(root, "InGameActions/AddUserInterfaces/File")
        .text()
        .unwrap();
    assert_eq!(ui, "UI/JAC_ReligionPersistence_HeroUI.xml");
    let ui_path = repo_root().join(MOD).join(ui);
    let ui_xml = std::fs::read_to_string(&ui_path).unwrap();
    let context = roxmltree::Document::parse(&ui_xml).unwrap();
    assert_eq!(context.root_element().tag_name().name(), "Context");
    assert_eq!(
        context.root_element().attribute("Name"),
        ui_path.file_stem().and_then(|x| x.to_str())
    );
    assert_eq!(
        context
            .root_element()
            .children()
            .filter(|n| n.is_element())
            .count(),
        0
    );
    assert_eq!(
        read_repo(format!("{MOD}/UI/JAC_ReligionPersistence_HeroUI.lua")),
        read_repo(format!("{MOD}/UI/JAC_ReligionPersistence_HeroUI.lua"))
    );
    let imports: BTreeSet<_> = path_node(root, "InGameActions/ImportFiles")
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "File")
        .map(|n| n.text().unwrap())
        .collect();
    assert_eq!(
        imports,
        BTreeSet::from([
            "Scripts/JAC_ReligionPersistence.lua",
            "Scripts/JAC_RP_AbsorptionAdapter.lua",
            "UI/JAC_ReligionPersistence_HeroUI.xml",
            "UI/JAC_ReligionPersistence_HeroUI.lua"
        ])
    );
    let db_order: i64 = path_node(root, "InGameActions/UpdateDatabase/Properties/LoadOrder")
        .text()
        .unwrap()
        .parse()
        .unwrap();
    let script_order: i64 = path_node(
        root,
        "InGameActions/AddGameplayScripts/Properties/LoadOrder",
    )
    .text()
    .unwrap()
    .parse()
    .unwrap();
    assert!(db_order < script_order);
}

#[test]
fn test_every_ingame_action_requires_banseiki_and_excludes_standard_gs() {
    let text = manifest_text();
    let doc = roxmltree::Document::parse(&text).unwrap();
    let root = doc.root_element();
    let criteria: BTreeMap<_, _> = path_node(root, "ActionCriteria")
        .children()
        .filter(|n| n.is_element())
        .map(|n| (n.attribute("id").unwrap(), n))
        .collect();
    let actions: Vec<_> = path_node(root, "InGameActions")
        .children()
        .filter(|n| n.is_element())
        .collect();
    assert_eq!(
        actions
            .iter()
            .map(|n| n.tag_name().name())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "UpdateDatabase",
            "UpdateText",
            "AddGameplayScripts",
            "AddUserInterfaces",
            "ImportFiles"
        ])
    );
    for action in actions {
        let criterion = criteria.get(action.attribute("criteria").unwrap()).unwrap();
        let rules = child(*criterion, "RuleSetInUse").text().unwrap();
        assert_eq!(
            rules.split(',').map(str::trim).collect::<Vec<_>>(),
            ["RULESET_JAC_BANSEIKI"]
        );
    }
    assert!(path_node(root, "Dependencies")
        .children()
        .filter(|n| n.is_element())
        .any(|n| n.attribute("id") == Some("bd7a82c5-6b38-4b69-a7f5-8dc9af534249")));
}

#[test]
fn test_fast_deadlines_are_gated_and_leave_production_values_intact() {
    let text = manifest_text();
    let doc = roxmltree::Document::parse(&text).unwrap();
    let root = doc.root_element();
    let action = path_node(root, "InGameActions")
        .children()
        .find(|n| n.attribute("id") == Some("JAC_RP_TestDeadlines"))
        .unwrap();
    let criterion = path_node(root, "ActionCriteria")
        .children()
        .find(|n| n.attribute("id") == action.attribute("criteria"))
        .unwrap();
    assert_eq!(
        path_node(criterion, "ConfigurationValueMatches/ConfigurationId").text(),
        Some("JAC_AUTOTEST")
    );
    assert_eq!(
        path_node(criterion, "ConfigurationValueMatches/Value").text(),
        Some("1")
    );
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("CREATE TABLE JAC_RP_Rules(ID TEXT PRIMARY KEY,Value INTEGER);INSERT INTO JAC_RP_Rules VALUES('REVIVAL_COOLDOWN_TURNS',10);CREATE TABLE GlobalParameters(Name TEXT PRIMARY KEY,Value INTEGER);INSERT INTO GlobalParameters VALUES('DIPLOMACY_ALLIANCE_TIME_LIMIT',30);").unwrap();
    db.execute_batch(&read_repo(format!(
        "{MOD}/{}",
        child(action, "File").text().unwrap()
    )))
    .unwrap();
    assert_eq!(
        db.query_row("SELECT Value FROM JAC_RP_Rules", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        db.query_row("SELECT Value FROM GlobalParameters", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        30
    );
    assert_eq!(
        catalog()["Rules"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["ID"] == "REVIVAL_COOLDOWN_TURNS")
            .unwrap()["Value"],
        10
    );
}

#[test]
fn test_all_catalog_and_state_names_have_ja_en_ru_text() {
    let text = read_repo(format!("{MOD}/Text/JAC_ReligionPersistence_Text.xml"));
    let doc = roxmltree::Document::parse(&text).unwrap();
    let rows: Vec<_> = doc
        .descendants()
        .filter(|n| n.is_element() && n.tag_name().name() == "Row")
        .collect();
    let texts: BTreeMap<_, _> = rows
        .iter()
        .map(|r| {
            (
                (
                    r.attribute("Tag").unwrap(),
                    r.attribute("Language").unwrap(),
                ),
                child(*r, "Text").text().unwrap_or(""),
            )
        })
        .collect();
    assert_eq!(rows.len(), texts.len());
    let data = catalog();
    let mut tags: BTreeSet<String> = TABLES
        .iter()
        .filter(|name| **name != "Rules")
        .flat_map(|name| {
            data[*name]
                .as_array()
                .unwrap()
                .iter()
                .map(|r| r["Name"].as_str().unwrap().to_owned())
        })
        .collect();
    for x in [
        "PUBLIC",
        "UNDERGROUND",
        "LOST",
        "DEFERRED",
        "FOLLOWERS",
        "FACILITIES",
        "FOUNDER_DEFEATED",
        "CAN_REVIVE",
        "APPLIED",
        "PENDING",
    ] {
        tags.insert(format!("LOC_JAC_RP_{x}"));
    }
    assert_eq!(tags.len(), 56);
    let expected: BTreeSet<_> = tags
        .iter()
        .flat_map(|tag| {
            ["ja_JP", "en_US", "ru_RU"]
                .into_iter()
                .map(move |lang| (tag.as_str(), lang))
        })
        .collect();
    assert_eq!(texts.keys().copied().collect::<BTreeSet<_>>(), expected);
    for value in texts.values() {
        assert!(!value.is_empty() && !value.starts_with("LOC_") && !value.contains('\u{fffd}'));
    }
    let manifest = manifest_text();
    let doc = roxmltree::Document::parse(&manifest).unwrap();
    for row in path_node(doc.root_element(), "LocalizedText")
        .children()
        .filter(|n| n.is_element() && n.tag_name().name() == "Text")
    {
        assert_eq!(
            row.children()
                .filter(|n| n.is_element())
                .map(|n| n.tag_name().name())
                .collect::<BTreeSet<_>>(),
            BTreeSet::from(["ja_JP", "en_US", "ru_RU"])
        );
    }
}

#[test]
fn test_fresh_game_and_script_reload_do_not_write_or_found_anything() {
    let h = Harness::new();
    assert_eq!(
        h.export(),
        json!({"schema":1,"revision":0,"religions":{},"heroes":{},"artifacts":{},"religiousAlliances":{}})
    );
    assert_eq!(h.eval(&format!("PROPERTY_STORE.{KEY}")), Json::Null);
    h.exec(&read_repo(format!(
        "{MOD}/Scripts/JAC_ReligionPersistence.lua"
    )));
    assert_eq!(h.writes(), 0);
}

#[test]
fn test_mechanism_accepts_a_replaced_religion_catalog() {
    let mut h = Harness::new();
    let mut data = catalog();
    data["Religions"] = json!([{"ID":"OTHER_CATALOG_FAITH","Name":"LOC_OTHER"}]);
    h.load(None, Some(data), None).unwrap();
    h.found("OTHER_CATALOG_FAITH", 3, 1, 0);
    assert_eq!(h.state("OTHER_CATALOG_FAITH")["followers"], 3);
    reject!(h, "Found", R1, 0, "origin", 3, 1, "PLAYER_0");
}

#[test]
fn test_founder_defeat_keeps_believers_assets_and_other_faction() {
    let mut h = Harness::new();
    h.f();
    call!(h, "SetCommunity", R1, "diaspora", 1, 0, "PLAYER_1");
    call!(h, "PreserveAsset", R1, "book", "SCRIPTURE", "EXILE_1");
    call!(h, "NotifyFounderDefeated", 0);
    assert_eq!(
        (h.s()["followers"].clone(), h.s()["state"].clone()),
        (json!(4).into(), json!("PUBLIC").into())
    );
    assert_python_truth(&h.s()["founderDefeated"], true);
    assert_eq!(h.s()["assets"]["book"]["holder"], "EXILE_1");
    h.l();
    assert_eq!(
        (h.s()["followers"].clone(), h.s()["state"].clone()),
        (json!(1).into(), json!("UNDERGROUND").into())
    );
    h.reload();
    let before = h.snapshot();
    call!(h, "NotifyFounderDefeated", 0);
    assert_eq!(h.snapshot(), before);
}

#[test]
fn test_global_last_follower_is_required_for_loss() {
    let h = Harness::new();
    h.found(R1, 1, 0, 0);
    call!(h, "SetCommunity", R1, "elsewhere", 1, 1, "PLAYER_1");
    h.l();
    assert_eq!(
        (h.s()["state"].clone(), h.s()["followers"].clone()),
        (json!("PUBLIC").into(), json!(1).into())
    );
    call!(h, "SetCommunity", R1, "elsewhere", 0, 1, "PLAYER_1");
    assert_eq!(
        (h.s()["state"].clone(), h.s()["followers"].clone()),
        (json!("LOST").into(), json!(0).into())
    );
    assert_eq!(h.s()["publicFacilities"], 1);
    assert!(!h.s()["canRevive"].python_truthy());
}

#[test]
fn test_underground_believers_need_no_public_facility_or_item() {
    let mut h = Harness::new();
    h.found(R1, 1, 0, 0);
    assert_eq!(h.s()["state"], "UNDERGROUND");
    assert!(!h.s()["lost"].python_truthy());
    h.reload();
}

#[test]
fn test_loss_retains_all_assets_and_underground_evidence() {
    let mut h = Harness::new();
    h.f();
    for kind in ["SCRIPTURE", "RELIC", "RECORD", "PERSON", "HOLY_SITE"] {
        call!(
            h,
            "PreserveAsset",
            R1,
            kind,
            kind,
            if kind == "PERSON" { "EXILE_1" } else { "CAVE" }
        );
    }
    let before = h.s()["assets"].clone();
    h.l();
    assert_eq!(h.s()["assets"], before);
    assert_eq!(h.s()["state"], "LOST");
    assert!(h.s()["underground"].python_truthy());
    assert!(h.s()["canRevive"].python_truthy());
    h.reload();
    assert_eq!(h.s()["followers"], 0);
}

#[test]
fn test_each_of_the_four_revival_sources_survives_load() {
    for kind in ["SCRIPTURE", "RELIC", "RECORD", "PERSON"] {
        let mut h = Harness::new();
        h.f();
        call!(h, "PreserveAsset", R1, "source", kind, "EXILE_1", 1);
        h.l();
        h.reload();
        call!(h, "Revive", R1, "revival", "source", "new", 1, 0);
        assert_eq!(
            (h.s()["state"].clone(), h.s()["followers"].clone()),
            (json!("UNDERGROUND").into(), json!(1).into()),
            "{kind}"
        );
        assert_eq!(h.s()["revivalCount"], 1);
        assert_eq!(h.s()["assets"]["source"]["kind"], kind);
        assert_eq!(h.engine("1")["faith"], 480);
        h.reload();
    }
}

#[test]
fn test_holy_site_alone_and_no_source_cannot_revive() {
    let mut h = Harness::new();
    h.f();
    h.l();
    reject!(h, "Revive", R1, "r", "absent", "new", 1, 0);
    call!(h, "PreserveAsset", R1, "site", "HOLY_SITE", "RUINS");
    assert!(!h.s()["canRevive"].python_truthy());
    reject!(h, "Revive", R1, "r", "site", "new", 1, 0);
    call!(h, "PreserveAsset", R1, "record", "RECORD", "SCHOLAR_1");
    assert!(h.s()["canRevive"].python_truthy());
    call!(h, "RemoveAsset", R1, "record");
    assert!(!h.s()["canRevive"].python_truthy());
    call!(h, "RemoveAsset", R1, "site");
    assert!(!h.s()["underground"].python_truthy());
    assert_eq!(h.s()["state"], "LOST");
    h.reload();
}

#[test]
fn test_item_transfer_preserves_identity_kind_and_revival_source() {
    let mut h = Harness::new();
    h.f();
    call!(h, "PreserveAsset", R1, "book", "SCRIPTURE", "PLAYER_0");
    call!(h, "MoveAsset", R1, "book", "EXILE_1");
    assert_eq!(
        h.s()["assets"]["book"],
        json!({"kind":"SCRIPTURE","holder":"EXILE_1"})
    );
    reject!(h, "PreserveAsset", R1, "book", "RELIC", "PLAYER_1");
    reject!(h, "MoveAsset", R1, "absent", "PLAYER_1");
    h.reload();
}

#[test]
fn test_revival_is_explicit_and_duplicate_revival_is_rejected() {
    let mut h = Harness::new();
    h.f();
    call!(h, "PreserveAsset", R1, "book", "SCRIPTURE", "PLAYER_1", 1);
    reject!(h, "Revive", R1, "r", "book", "new", 1, 0);
    h.l();
    reject!(h, "Revive", R1, "r", "book", "new", 0, 0);
    call!(h, "Revive", R1, "r", "book", "new", 1, 0);
    assert_eq!(h.s()["state"], "UNDERGROUND");
    reject!(h, "Revive", R1, "r", "book", "new", 1, 0);
    call!(h, "SetCommunity", R1, "new", 0, 0, "PLAYER_1");
    reject!(h, "Revive", R1, "r2", "book", "new2", 1, 0);
    h.exec("TURN=TURN+10");
    call!(h, "Revive", R1, "r2", "book", "new2", 1, 0);
    assert_eq!(h.s()["revivalCount"], 2);
    assert_eq!(h.engine("1")["faith"], 460);
    h.reload();
    assert_eq!(
        h.s()["revivals"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["r", "r2"])
    );
}

#[test]
fn test_revival_picks_a_free_slot_and_preserves_both_religions() {
    for discovered in [false, true] {
        let mut h = Harness::new();
        h.f();
        h.found(R2, 5, 4, 0);
        call!(h, "SetCommunity", R1, "origin", 0, 2, "PLAYER_0");
        call!(h, "SetCommunity", R1, "origin:revival_1", 0, 3, "ARCHIVE");
        if discovered {
            h.d();
        } else {
            call!(h, "PreserveAsset", R1, "book", "SCRIPTURE", "PLAYER_1", 1);
        }
        h.reload();
        let original = h.s()["communities"].clone();
        let other = h.state(R2);
        if discovered {
            h.resolve(R1, "d", "REVIVE", h.options(json!({"community":"origin"})));
        } else {
            call!(h, "Revive", R1, "r", "book", "origin", 1, 0);
        }
        let revived = h.s();
        let selected = revived["revivals"]["r"]["community"].as_str().unwrap();
        assert!(!original.as_object().unwrap().contains_key(selected));
        for (key, value) in original.as_object().unwrap() {
            assert_eq!(&revived["communities"][key], value);
        }
        assert_eq!(
            revived["communities"][selected],
            json!({"followers":1,"publicFacilities":0,"holder":"PLAYER_1"})
        );
        assert_eq!(
            (
                revived["followers"].clone(),
                revived["publicFacilities"].clone(),
                revived["state"].clone()
            ),
            (json!(1).into(), json!(5).into(), json!("PUBLIC").into())
        );
        assert_eq!(h.state(R2), other);
        assert_eq!(h.engine("1")["faith"], 480);
        h.reload();
        assert_eq!(h.s(), revived);
        assert_eq!(h.state(R2), other);
    }
}

#[test]
fn test_revival_does_not_overwrite_a_slot_created_by_the_faith_callback() {
    let mut h = Harness::new();
    h.f();
    h.l();
    call!(h, "PreserveAsset", R1, "book", "SCRIPTURE", "PLAYER_1", 1);
    h.exec("local religion=Players[1]:GetReligion(); local change=religion.ChangeFaithBalance; religion.ChangeFaithBalance=function(self,amount) change(self,amount); rp.SetCommunity('JAC_RP_PLACEHOLDER_01','new',0,7,'PLAYER_0') end");
    call!(h, "Revive", R1, "r", "book", "new", 1, 0);
    let revived = h.s();
    let selected = revived["revivals"]["r"]["community"].as_str().unwrap();
    assert_ne!(selected, "new");
    assert_eq!(
        revived["communities"]["new"],
        json!({"followers":0,"publicFacilities":7,"holder":"PLAYER_0"})
    );
    assert_eq!(
        revived["communities"][selected],
        json!({"followers":1,"publicFacilities":0,"holder":"PLAYER_1"})
    );
    assert_eq!(
        (
            revived["followers"].clone(),
            revived["publicFacilities"].clone(),
            revived["revivalCount"].clone()
        ),
        (json!(1).into(), json!(7).into(), json!(1).into())
    );
    assert_eq!(h.engine("1")["faith"], 480);
    h.reload();
    assert_eq!(h.s(), revived);
}

#[test]
fn test_revival_free_slot_respects_the_identifier_length_limit() {
    let mut h = Harness::new();
    let preferred = "x".repeat(128);
    call!(h, "Found", R1, 0, preferred, 1, 1, "PLAYER_0");
    call!(h, "SetCommunity", R1, preferred, 0, 1, "PLAYER_0");
    call!(h, "PreserveAsset", R1, "book", "SCRIPTURE", "PLAYER_1", 1);
    call!(h, "Revive", R1, "r", "book", preferred, 1, 0);
    let state = h.s();
    let selected = state["revivals"]["r"]["community"].as_str().unwrap();
    assert_ne!(selected, preferred);
    assert!(selected.len() <= 128);
    assert_eq!(
        state["communities"][&preferred],
        json!({"followers":0,"publicFacilities":1,"holder":"PLAYER_0"})
    );
    h.reload();
}

#[test]
fn test_discovery_accepts_all_listed_jobs_sites_and_both_item_kinds() {
    let mut h = Harness::new();
    h.f();
    h.l();
    let mut n = 0;
    for job in h.tables["Jobs"].as_array().unwrap() {
        for site in h.tables["Sites"].as_array().unwrap() {
            for kind in ["SCRIPTURE", "RELIC"] {
                n += 1;
                call!(
                    h,
                    "Discover",
                    R1,
                    format!("d{n}"),
                    format!("item{n}"),
                    kind,
                    "ACTOR_1",
                    job["ID"],
                    site["ID"],
                    1
                );
            }
        }
    }
    assert_eq!(n, 70);
    assert_eq!(h.s()["discoveries"].as_object().unwrap().len(), 70);
    assert_eq!(h.s()["assets"].as_object().unwrap().len(), 70);
    h.reload();
}

#[test]
fn test_discovery_transfers_existing_item_and_rejects_wrong_site_and_replays() {
    let mut h = Harness::new();
    h.f();
    call!(h, "PreserveAsset", R1, "book", "SCRIPTURE", "CAVE");
    reject!(
        h,
        "Discover",
        R1,
        "d",
        "book",
        "SCRIPTURE",
        "MONK_1",
        "MONK",
        "CAVE",
        1
    );
    h.l();
    reject!(
        h,
        "Discover",
        R1,
        "d",
        "book",
        "SCRIPTURE",
        "MONK_1",
        "MONK",
        "RUINS",
        1
    );
    reject!(h, "Discover", R1, "d", "new", "RECORD", "MONK_1", "MONK", "CAVE", 1);
    reject!(
        h,
        "Discover",
        R1,
        "d",
        "new",
        "SCRIPTURE",
        "MONK_1",
        "UNKNOWN",
        "CAVE",
        1
    );
    reject!(
        h,
        "Discover",
        R1,
        "d",
        "new",
        "SCRIPTURE",
        "MONK_1",
        "MONK",
        "UNKNOWN",
        1
    );
    call!(
        h,
        "Discover",
        R1,
        "d",
        "book",
        "SCRIPTURE",
        "MONK_1",
        "MONK",
        "CAVE",
        1
    );
    assert_eq!(h.s()["assets"]["book"]["holder"], "MONK_1");
    reject!(
        h,
        "Discover",
        R1,
        "d",
        "book",
        "SCRIPTURE",
        "MONK_1",
        "MONK",
        "CAVE",
        1
    );
    h.reload();
}

#[test]
fn test_every_outcome_applies_the_exact_reward_and_survives_load_without_replay() {
    let outcomes: Vec<String> = catalog()["Outcomes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["ID"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(
        outcomes.iter().map(String::as_str).collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "REVIVE",
            "SELL",
            "CONCEAL",
            "RESEARCH",
            "HERESY",
            "ABSORB",
            "ARTIFACT",
            "DIPLOMATIC_INCIDENT",
            "HERO"
        ])
    );
    let rewards = json!({"SELL":{"gold":500},"RESEARCH":{"science":10,"culture":10},"HERESY":{"culture":20,"faith":-400},"ABSORB":{"faith":-500},"ARTIFACT":{"culture":30},"DIPLOMATIC_INCIDENT":{"favor":5,"gold":50},"REVIVE":{"faith":-20},"HERO":{"faith":32.5}});
    for outcome in outcomes {
        let mut h = Harness::new();
        h.f();
        h.l();
        h.found(R2, 3, 1, 0);
        call!(h, "SetCommunity", R2, "origin", 3, 1, "PLAYER_1");
        h.d();
        if outcome == "ABSORB" {
            h.bind_absorption();
        }
        let before = h.engine("1");
        h.resolve(R1, "d", &outcome, json!({}));
        if outcome == "HERO" {
            h.exec("HERO_UI_TICK()");
        }
        let state = h.s();
        let d = &state["discoveries"]["d"];
        assert_eq!(d["outcome"], outcome);
        assert_eq!(d["effects"], "APPLIED");
        assert_eq!(
            state["followers"],
            if outcome == "REVIVE" {
                1
            } else if outcome == "ABSORB" {
                4
            } else {
                0
            }
        );
        assert_eq!(
            state["revivalCount"],
            if outcome == "REVIVE" { 1 } else { 0 }
        );
        let after = h.engine("1");
        for yield_name in ["gold", "faith", "science", "culture", "favor"] {
            let delta =
                after[yield_name].python_f64().unwrap() - before[yield_name].python_f64().unwrap();
            assert_eq!(
                delta,
                rewards[&outcome][yield_name].as_f64().unwrap_or(0.0),
                "{outcome} {yield_name}"
            );
        }
        assert_eq!(
            after["units"].as_object().unwrap().len(),
            if outcome == "HERO" { 1 } else { 0 }
        );
        if outcome == "ABSORB" {
            assert!(state["assets"].get("book").is_none());
            assert_eq!(h.state(R2)["assets"]["book"]["originReligion"], R1);
        }
        if outcome == "ARTIFACT" {
            assert!(state["assets"]["book"]["vault"].python_truthy());
        }
        if outcome == "HERESY" {
            assert!(state["assets"].get("book").is_none());
            assert_eq!(state["materialUses"]["book"]["interpretation"], "HERETICAL");
        }
        if outcome == "RESEARCH" {
            assert!(state["assets"].get("book").is_none());
            assert_eq!(after["boosted"], json!({"2":true,"4":true}));
        }
        if outcome == "DIPLOMATIC_INCIDENT" {
            assert_eq!(state["assets"]["book"]["owner"], 0);
        }
        h.reload();
        assert_eq!(h.engine("1"), after);
        h.rejected(
            "ResolveDiscovery",
            vec![json!(R1), json!("d"), json!(outcome), h.options(json!({}))],
        );
        assert_eq!(h.engine("1"), after);
    }
}

#[test]
fn test_removed_discovery_source_prevents_revival_without_marking_resolution() {
    let h = Harness::new();
    h.f();
    h.l();
    h.d();
    call!(h, "RemoveAsset", R1, "book");
    h.rejected(
        "ResolveDiscovery",
        vec![json!(R1), json!("d"), json!("REVIVE"), h.options(json!({}))],
    );
    assert!(h.s()["discoveries"]["d"].get("outcome").is_none());
}

#[test]
fn test_direct_revival_cannot_strand_an_unresolved_discovery() {
    for kind in ["SCRIPTURE", "RELIC"] {
        let mut h = Harness::new();
        h.f();
        h.l();
        call!(h, "Discover", R1, "d", "book", kind, "MONK_1", "MONK", "CAVE", 1);
        h.reload();
        let before = h.eval("ENGINE");
        reject!(h, "Revive", R1, "direct", "book", "new", 1, 0);
        assert_eq!(h.eval("ENGINE"), before);
        assert!(h.s()["discoveries"]["d"].get("outcome").is_none());
        h.resolve(R1, "d", "REVIVE", json!({}));
        let discovery = h.s()["discoveries"]["d"].clone();
        assert_eq!(
            (discovery["outcome"].clone(), discovery["effects"].clone()),
            (json!("REVIVE").into(), json!("APPLIED").into())
        );
        assert_eq!(h.engine("1")["faith"], 480);
        h.reload();
    }
}

#[test]
fn test_research_chooses_two_new_eurekas_in_type_order_and_consumes_material() {
    let mut h = Harness::new();
    h.f();
    h.l();
    h.d();
    h.exec("ENGINE['1'].researched['2']=true; ENGINE['1'].boosted['4']=true");
    let before = h.engine("1");
    h.resolve(R1, "d", "RESEARCH", json!({}));
    assert_eq!(
        h.engine("1")["boosted"],
        json!({"4":true,"3":true,"1":true})
    );
    assert_eq!(h.s()["discoveries"]["d"]["effect"]["boosts"], json!([3, 1]));
    assert_eq!(
        h.engine("1")["science"].python_f64().unwrap() - before["science"].python_f64().unwrap(),
        10.0
    );
    assert_eq!(
        h.engine("1")["culture"].python_f64().unwrap() - before["culture"].python_f64().unwrap(),
        10.0
    );
    assert!(h.s()["retiredAssets"]["book"].python_truthy());
    reject!(h, "PreserveAsset", R1, "book", "SCRIPTURE", "CAVE", 1);
    h.reload();
}

#[test]
fn test_research_requires_two_distinct_eligible_boosts_before_any_effect() {
    let h = Harness::new();
    h.f();
    h.l();
    h.d();
    h.exec("for i=1,3 do ENGINE['1'].boosted[tostring(i)]=true end");
    let before = h.eval("ENGINE");
    h.rejected(
        "ResolveDiscovery",
        vec![
            json!(R1),
            json!("d"),
            json!("RESEARCH"),
            h.options(json!({})),
        ],
    );
    assert_eq!(h.eval("ENGINE"), before);
}

#[test]
fn test_research_eurekas_precede_science_that_finishes_a_chosen_tech() {
    let mut h = Harness::new();
    h.f();
    h.l();
    h.d();
    h.exec("local p=Players[1]; local attach=p.AttachModifierByID; p.AttachModifierByID=function(self,id) if id=='JAC_RP_RESEARCH_JAC_RP_PLACEHOLDER_01' then assert(ENGINE['1'].boosted['2'] and ENGINE['1'].boosted['4']); ENGINE['1'].researched['2']=true end; attach(self,id) end");
    h.resolve(R1, "d", "RESEARCH", json!({}));
    assert_eq!(h.engine("1")["boosted"], json!({"2":true,"4":true}));
    assert_eq!(
        (
            h.engine("1")["science"].clone(),
            h.engine("1")["culture"].clone()
        ),
        (json!(10).into(), json!(10).into())
    );
    h.reload();
}

#[test]
fn test_heresy_requires_400_faith_and_consumes_material() {
    let mut h = Harness::new();
    h.f();
    h.l();
    h.d();
    h.exec("ENGINE['1'].faith=399");
    let before = h.eval("ENGINE");
    h.rejected(
        "ResolveDiscovery",
        vec![json!(R1), json!("d"), json!("HERESY"), h.options(json!({}))],
    );
    assert_eq!(h.eval("ENGINE"), before);
    h.exec("ENGINE['1'].faith=400");
    h.resolve(R1, "d", "HERESY", json!({}));
    assert_eq!(
        (
            h.engine("1")["faith"].clone(),
            h.engine("1")["culture"].clone()
        ),
        (json!(0).into(), json!(20).into())
    );
    assert!(h.s()["assets"].get("book").is_none());
    assert!(h.s()["retiredAssets"]["book"].python_truthy());
    reject!(h, "Revive", R1, "r", "book", "new", 1, 0);
    h.reload();
}

#[test]
fn test_absorption_rounds_once_worldwide_and_preserves_other_religions() {
    let mut h = Harness::new();
    h.f();
    h.l();
    h.found(R2, 3, 1, 0);
    call!(h, "SetCommunity", R2, "origin", 1, 0, "PLAYER_1");
    h.d();
    h.bind_absorption();
    h.exec("local a,b=ENGINE['0'].cities['0'],ENGINE['1'].cities['0']; a.population=2; a.pressure={['42']=1024,['44']=1024}; b.population=5; b.pressure={['42']=4096,['43']=1024}; ENGINE['1'].faith=499");
    let before = h.eval("ENGINE");
    h.rejected(
        "ResolveDiscovery",
        vec![json!(R1), json!("d"), json!("ABSORB"), h.options(json!({}))],
    );
    assert_eq!(h.eval("ENGINE"), before);
    h.exec("ENGINE['1'].faith=500");
    h.resolve(R1, "d", "ABSORB", json!({}));
    let effect = h.s()["discoveries"]["d"]["effect"].clone();
    assert_eq!(
        (
            effect["sourceFollowers"].clone(),
            effect["convertedFollowers"].clone()
        ),
        (json!(5).into(), json!(2).into())
    );
    let conversions: Vec<_> = effect["conversions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| {
            Data::Array(vec![
                c["player"].clone(),
                c["city"].clone(),
                c["moved"].clone(),
            ])
        })
        .collect();
    assert_eq!(conversions, vec![json!([0, 0, 1]), json!([1, 0, 1])]);
    assert_eq!(
        (
            call!(h, "NativeFollowers", R1),
            call!(h, "NativeFollowers", R2)
        ),
        (json!(3).into(), json!(3).into())
    );
    let rows = h.eval("NativeRows(ENGINE['0'].cities['0'])");
    let counts: BTreeMap<_, _> = rows
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r["Religion"].python_i64().unwrap(),
                r["Followers"].python_i64().unwrap(),
            )
        })
        .collect();
    assert_eq!(counts, BTreeMap::from([(43, 1), (44, 1)]));
    assert_eq!(h.engine("1")["faith"], 0);
    h.reload();
}

#[test]
fn test_absorption_rejects_unbound_or_foreign_native_target() {
    let h = Harness::new();
    h.f();
    h.l();
    h.found(R2, 3, 1, 0);
    call!(h, "SetCommunity", R2, "origin", 1, 0, "PLAYER_1");
    h.d();
    h.rejected(
        "ResolveDiscovery",
        vec![json!(R1), json!("d"), json!("ABSORB"), h.options(json!({}))],
    );
    call!(h, "BindEngineReligion", R1, 42);
    call!(h, "BindEngineReligion", R2, 44);
    h.rejected(
        "ResolveDiscovery",
        vec![json!(R1), json!("d"), json!("ABSORB"), h.options(json!({}))],
    );
    reject!(h, "BindEngineReligion", R2, 43);
}

#[test]
fn test_absorption_clears_maximum_fixed_point_pressure_including_fraction() {
    let h = Harness::new();
    h.f();
    h.l();
    h.found(R2, 3, 1, 0);
    h.d();
    h.bind_absorption();
    h.exec("ENGINE['0'].cities['0'].pressure={['42']=8388607+255/256}; ENGINE['1'].cities['0'].pressure={['43']=1024}");
    h.resolve(R1, "d", "ABSORB", json!({}));
    assert_eq!(
        (
            call!(h, "NativeFollowers", R1),
            call!(h, "NativeFollowers", R2)
        ),
        (json!(4).into(), json!(7).into())
    );
    assert_eq!(h.engine("1")["faith"], 0);
}

#[test]
fn test_absorption_preserves_unaffiliated_citizens_even_when_row_is_omitted() {
    for (omitted, sentinel) in [(false, -1), (true, -1), (false, 0), (true, 0)] {
        let h = Harness::new();
        h.f();
        h.l();
        h.found(R2, 3, 1, 0);
        call!(h, "SetCommunity", R2, "origin", 1, 0, "PLAYER_1");
        h.d();
        h.bind_absorption();
        h.exec(&format!("OMIT_UNAFFILIATED_ROWS={omitted}; local a,b=ENGINE['0'].cities['0'],ENGINE['1'].cities['0']; a.population=3; a.pressure={{['42']=256,['44']=256,['-1']=256}}; b.population=5; b.pressure={{['42']=4096,['43']=1024}}"));
        if sentinel == 0 {
            h.exec("local c=ENGINE['0'].cities['0']; c.pressure['0']=c.pressure['-1']; c.pressure['-1']=nil");
        }
        h.exec("ENGINE['1'].faith=500");
        h.resolve(R1, "d", "ABSORB", json!({}));
        assert_eq!(
            (
                call!(h, "NativeFollowers", R1),
                call!(h, "NativeFollowers", R2)
            ),
            (json!(3).into(), json!(3).into()),
            "{omitted} {sentinel}"
        );
        assert_eq!(
            h.eval(&format!("ENGINE['0'].cities['0'].pressure['{sentinel}']")),
            256
        );
        h.exec("OMIT_UNAFFILIATED_ROWS=false");
        let rows = h.eval("NativeRows(ENGINE['0'].cities['0'])");
        let counts: BTreeMap<_, _> = rows
            .as_array()
            .unwrap()
            .iter()
            .map(|r| {
                (
                    r["Religion"].python_i64().unwrap(),
                    r["Followers"].python_i64().unwrap(),
                )
            })
            .collect();
        assert_eq!(counts, BTreeMap::from([(43, 1), (44, 1), (sentinel, 1)]));
        assert_eq!(h.engine("1")["faith"], 0);
    }
}

#[test]
fn test_bound_native_counts_prevent_false_loss_and_revival() {
    let mut h = Harness::new();
    h.f();
    h.l();
    call!(h, "PreserveAsset", R1, "book", "SCRIPTURE", "PLAYER_1", 1);
    call!(h, "BindEngineReligion", R1, 42);
    assert_eq!(h.s()["followers"], 7);
    assert!(!h.s()["lost"].python_truthy());
    reject!(
        h,
        "Discover",
        R1,
        "d",
        "book",
        "SCRIPTURE",
        "MONK_1",
        "MONK",
        "CAVE",
        1
    );
    reject!(h, "Revive", R1, "r", "book", "new", 1, 0);
    h.l();
    assert_eq!(h.s()["followers"], 7);
    h.reload();
}

#[test]
fn test_zero_native_absorption_does_not_charge_or_reserve() {
    let h = Harness::new();
    h.f();
    h.l();
    h.d();
    h.found(R2, 3, 1, 0);
    call!(h, "SetCommunity", R2, "origin", 1, 0, "PLAYER_1");
    h.exec("ENGINE['0'].cities['0'].pressure={['43']=7168}");
    h.bind_absorption();
    h.exec("ENGINE['1'].faith=500");
    h.rejected(
        "ResolveDiscovery",
        vec![json!(R1), json!("d"), json!("ABSORB"), h.options(json!({}))],
    );
    assert_eq!(h.engine("1")["faith"], 500);
}

#[test]
fn test_stale_owned_community_cannot_authorize_absorption() {
    let h = Harness::new();
    h.f();
    h.l();
    h.d();
    h.found(R2, 3, 1, 0);
    call!(h, "SetCommunity", R2, "origin", 1, 0, "PLAYER_1");
    h.exec("ENGINE['0'].cities['0'].pressure={['42']=4096,['43']=3072}; ENGINE['1'].cities['0'].pressure={['42']=4096}");
    h.bind_absorption();
    h.rejected(
        "ResolveDiscovery",
        vec![json!(R1), json!("d"), json!("ABSORB"), h.options(json!({}))],
    );
}

#[test]
fn test_actual_owned_followers_authorize_absorption_with_zero_manual_count() {
    let h = Harness::new();
    h.f();
    h.l();
    h.d();
    h.found(R2, 3, 1, 0);
    call!(h, "SetCommunity", R2, "origin", 0, 0, "PLAYER_0");
    h.bind_absorption();
    h.resolve(R1, "d", "ABSORB", json!({}));
    assert_eq!(call!(h, "NativeFollowers", R1), 4);
}

#[test]
fn test_native_revival_uses_numeric_religion_order() {
    let h = Harness::new();
    h.f();
    h.l();
    h.d();
    h.exec("ENGINE['0'].cities['0'].pressure={['43']=7168}; ENGINE['1'].cities['0'].pressure={['2']=2048,['10']=2048}");
    call!(h, "BindEngineReligion", R1, 42);
    h.resolve(R1, "d", "REVIVE", json!({}));
    let e = h.s()["discoveries"]["d"]["effect"]["conversions"][0].clone();
    assert_eq!(
        (
            e["after"]["2"].clone(),
            e["after"]["10"].clone(),
            e["after"]["42"].clone()
        ),
        (json!(1).into(), json!(2).into(), json!(1).into())
    );
}

#[test]
fn test_bound_revival_creates_one_native_citizen_and_survives_load() {
    let mut h = Harness::new();
    h.f();
    h.l();
    h.d();
    h.exec("ENGINE['0'].cities['0'].pressure={['43']=7168}; ENGINE['1'].cities['0'].pressure={['43']=4096}");
    call!(h, "BindEngineReligion", R1, 42);
    assert!(h.s()["lost"].python_truthy());
    let before = h.engine("1")["faith"].python_f64().unwrap();
    h.resolve(R1, "d", "REVIVE", json!({}));
    assert_eq!(call!(h, "NativeFollowers", R1), 1);
    assert_eq!(h.s()["followers"], 1);
    assert!(!h.s()["lost"].python_truthy());
    assert_eq!(h.engine("1")["faith"].python_f64().unwrap(), before - 20.0);
    h.reload();
}

#[test]
fn test_inexact_engine_conversion_stays_pending_and_never_replays() {
    let mut h = Harness::new();
    h.f();
    h.l();
    h.found(R2, 3, 1, 0);
    call!(h, "SetCommunity", R2, "origin", 1, 0, "PLAYER_1");
    h.d();
    h.bind_absorption();
    h.exec("BAD_CONVERSION=true");
    let error = h
        .raw_call(
            "ResolveDiscovery",
            vec![json!(R1), json!("d"), json!("ABSORB"), h.options(json!({}))],
        )
        .unwrap_err();
    assert!(
        error.to_string().contains("conversion count mismatch"),
        "{error}"
    );
    assert_eq!(h.s()["discoveries"]["d"]["effects"], "PENDING");
    assert_eq!(h.engine("1")["faith"], 500);
    h.reload();
    h.rejected(
        "ResolveDiscovery",
        vec![json!(R1), json!("d"), json!("ABSORB"), h.options(json!({}))],
    );
}

#[test]
fn test_restitution_transfers_half_balances_and_prohibits_persecution_both_ways() {
    let mut h = Harness::new();
    h.f();
    h.l();
    h.found(R2, 3, 1, 0);
    h.d();
    h.exec("ENGINE['0'].gold=101.5; ENGINE['0'].favor=11");
    h.resolve(R1, "d", "DIPLOMATIC_INCIDENT", json!({}));
    assert_eq!(
        (h.engine("0")["gold"].clone(), h.engine("1")["gold"].clone()),
        (json!(50.75).into(), json!(150.75).into())
    );
    assert_eq!(
        (
            h.engine("0")["favor"].clone(),
            h.engine("1")["favor"].clone()
        ),
        (json!(6).into(), json!(15).into())
    );
    assert!(!call!(h, "CanPersecute", 0, R1, 1, R2)[0].python_truthy());
    assert!(!call!(h, "CanPersecute", 1, R2, 0, R1)[0].python_truthy());
    assert!(h.call_truth(
        "CanPersecute",
        vec![json!(0), json!(R2), json!(1), json!(R2)]
    ));
    let error = h
        .raw_call(
            "AssertCanPersecute",
            vec![json!(1), json!(R2), json!(0), json!(R1)],
        )
        .unwrap_err();
    assert!(error.to_string().contains("alliance prohibits"));
    h.reload();
    assert!(!call!(h, "CanPersecute", 0, R1, 1, R2)[0].python_truthy());
}

#[test]
fn test_religious_alliance_ends_on_diplomatic_end_war_or_defeat_and_does_not_restart() {
    for cause in ["allied", "war", "alive"] {
        let mut h = Harness::new();
        h.f();
        h.l();
        h.found(R2, 3, 1, 0);
        h.d();
        h.resolve(R1, "d", "DIPLOMATIC_INCIDENT", json!({}));
        h.exec(&format!(
            "ENGINE['1'].{cause}={}; TURN=TURN+1; TURN_HOOK()",
            if cause == "war" { "true" } else { "false" }
        ));
        let alliances = h.export()["religiousAlliances"].clone();
        let a = alliances.as_object().unwrap().values().next().unwrap();
        assert_eq!(
            (a["active"].clone(), a["endedTurn"].clone()),
            (json!(false).into(), h.eval("TURN")),
            "{cause}"
        );
        h.exec(&format!(
            "ENGINE['1'].{cause}={}",
            if cause == "war" { "false" } else { "true" }
        ));
        h.reload();
        assert!(h.call_truth(
            "CanPersecute",
            vec![json!(0), json!(R1), json!(1), json!(R2)]
        ));
        assert!(h.call_truth(
            "CanPersecute",
            vec![json!(1), json!(R2), json!(0), json!(R1)]
        ));
    }
}

fn pending_hero(h: &Harness) {
    h.f();
    h.l();
    h.d();
    h.resolve(R1, "d", "HERO", json!({}));
}
fn hero_verification(h: &Harness) -> Data {
    h.export()["heroes"]["1"]["verification"].clone()
}

fn network_request(h: &Harness, action: &str, extra: Json) -> Json {
    let mut request = json!({"Action":action,"Religion":R1,"Discovery":"d",
        "Turn":h.eval("TURN").python_f64().unwrap() as u64,
        "Revision":h.eval("rp.Export().revision").python_f64().unwrap() as u64});
    for (key, value) in extra.as_object().unwrap() {
        request[key] = value.clone();
    }
    request
}

fn network_event(h: &Harness, actor: i64, request: &Json) -> bool {
    h.lua
        .globals()
        .get::<Function>("RELIGION_OPERATION")
        .unwrap()
        .call::<bool>((actor, json_to_lua(&h.lua, request)))
        .unwrap()
}

fn network_discovery() -> Harness {
    let h = Harness::new();
    h.f();
    h.l();
    h.d();
    h.exec("MULTIPLAYER=true");
    h
}

fn assert_network_unchanged(h: &Harness, actor: i64, request: &Json) {
    let before = (h.snapshot(), h.eval("ENGINE"), h.writes());
    assert!(
        !network_event(h, actor, request),
        "accepted {actor}: {request}"
    );
    assert_eq!((h.snapshot(), h.eval("ENGINE"), h.writes()), before);
}

#[test]
fn multiplayer_independent_lua51_clients_apply_all_nonhero_choices_and_replay_once() {
    for outcome in [
        "SELL",
        "CONCEAL",
        "RESEARCH",
        "HERESY",
        "ARTIFACT",
        "REVIVE",
        "DIPLOMATIC_INCIDENT",
        "ABSORB",
    ] {
        let mut a = network_discovery();
        let mut b = network_discovery();
        for h in [&a, &b] {
            h.found(R2, 3, 1, 0);
            if outcome == "ABSORB" {
                h.bind_absorption();
            }
            // Native Hero observations and local player differ across peers.
            // None may enter a non-Hero decision or native reward.
            h.exec("ExposedMembers.JAC_RP_HeroUI.ClaimPlayer=function() error('UI read in Gameplay') end; ExposedMembers.JAC_RP_HeroUI.OriginBuilding=function() error('UI read in Gameplay') end; ExposedMembers.JAC_RP_HeroUI.UnitInfo=function() error('UI read in Gameplay') end");
        }
        a.exec("LOCAL_PLAYER=0");
        b.exec("LOCAL_PLAYER=1");
        let request = network_request(
            &a,
            "RESOLVE",
            json!({"Outcome":outcome,
            "City":0,"Revival":"r","Community":"new","TargetReligion":R2,"TargetPlayer":0}),
        );
        for h in [&a, &b] {
            assert!(network_event(h, 1, &request), "{outcome}");
        }
        assert_eq!(a.snapshot(), b.snapshot(), "{outcome}");
        assert_eq!(a.eval("ENGINE"), b.eval("ENGINE"), "{outcome}");
        for h in [&mut a, &mut b] {
            assert_network_unchanged(h, 1, &request);
            h.reload();
            h.exec("MULTIPLAYER=true");
            assert_network_unchanged(h, 1, &request);
            // A fresh revision cannot bypass the persisted domain receipt.
            let fresh = network_request(
                h,
                "RESOLVE",
                json!({"Outcome":outcome,"City":0,
                "Revival":"r","Community":"new","TargetReligion":R2,"TargetPlayer":0}),
            );
            assert_network_unchanged(h, 1, &fresh);
        }
        assert_eq!(a.snapshot(), b.snapshot());
        assert_eq!(a.eval("ENGINE"), b.eval("ENGINE"));
    }
}

#[test]
fn multiplayer_rejects_foreign_actor_spoofed_owner_and_stale_requests_without_effects() {
    let h = network_discovery();
    let request = network_request(&h, "RESOLVE", json!({"Outcome":"SELL"}));
    for actor in [-1, 0, 2] {
        assert_network_unchanged(&h, actor, &request);
    }
    for (key, value) in [
        ("PlayerID", json!(1)),
        ("Player", json!(1)),
        ("Turn", json!(16)),
        ("Revision", json!(0)),
        ("Action", json!("FOUND")),
        ("Outcome", json!({"nested":"SELL"})),
        ("ObservedClaim", json!(1)),
        ("Revision", json!(1.5)),
        ("Turn", json!("17")),
    ] {
        let mut invalid = request.clone();
        invalid[key] = value;
        assert_network_unchanged(&h, 1, &invalid);
    }
    h.exec("RELIGION_TURN_END(1)");
    assert_network_unchanged(&h, 1, &request);
    h.exec("RELIGION_TURN_START(1); HUMAN_ONE=false; MULTIPLAYER=false");
    assert_network_unchanged(&h, 1, &request);
    h.exec("HUMAN_ONE=true");
    assert!(network_event(&h, 1, &request));
    assert_eq!(h.engine("1")["gold"], 600);
}

#[test]
fn multiplayer_stale_revision_and_moved_material_cannot_be_resolved() {
    let h = network_discovery();
    let stale = network_request(&h, "RESOLVE", json!({"Outcome":"SELL"}));
    call!(h, "MoveAsset", R1, "book", "PLAYER_0", 0);
    assert_network_unchanged(&h, 1, &stale);
    let fresh = network_request(&h, "RESOLVE", json!({"Outcome":"SELL"}));
    assert_network_unchanged(&h, 1, &fresh);
}

#[test]
fn multiplayer_ui_only_queues_local_actor_and_waits_for_engine_delivery() {
    let h = network_discovery();
    h.exec("DEFER_OPERATIONS=true");
    let before = (h.snapshot(), h.eval("ENGINE"), h.writes());
    h.exec("assert(ExposedMembers.JAC_RP_HeroUI.ResolveDiscovery('JAC_RP_PLACEHOLDER_01','d','SELL',{player=0}))");
    assert_eq!((h.snapshot(), h.eval("ENGINE"), h.writes()), before);
    assert_eq!(h.eval("#REQUESTS"), 1);
    assert_eq!(h.eval("REQUESTS[1].actor"), 1);
    assert!(h.truth("REQUESTS[1].params.PlayerID==nil and REQUESTS[1].params.player==nil"));
    h.exec("local r=REQUESTS[1]; assert(RELIGION_OPERATION(r.actor,r.params)); assert(not RELIGION_OPERATION(r.actor,r.params))");
    assert_eq!(h.engine("1")["gold"], 600);
    h.exec("LOCAL_PLAYER=-1; assert(not ExposedMembers.JAC_RP_HeroUI.ResolveDiscovery('JAC_RP_PLACEHOLDER_01','d','SELL')); LOCAL_PLAYER=1; ENGINE['1'].active=false; assert(not ExposedMembers.JAC_RP_HeroUI.ResolveDiscovery('JAC_RP_PLACEHOLDER_01','d','SELL'))");
    assert_eq!(h.eval("#REQUESTS"), 1);
}

#[test]
fn multiplayer_all_eight_choices_traverse_ui_player_operation_and_two_engine_receivers() {
    for outcome in [
        "SELL",
        "CONCEAL",
        "RESEARCH",
        "HERESY",
        "ARTIFACT",
        "REVIVE",
        "DIPLOMATIC_INCIDENT",
        "ABSORB",
    ] {
        let a = network_discovery();
        let b = network_discovery();
        for h in [&a, &b] {
            h.found(R2, 3, 1, 0);
            if outcome == "ABSORB" {
                h.bind_absorption();
            }
        }
        a.exec("LOCAL_PLAYER=1; DEFER_OPERATIONS=true");
        b.exec("LOCAL_PLAYER=0; ExposedMembers.JAC_RP_HeroUI.UnitInfo=function() error('foreign UI observation') end");
        let before = (a.snapshot(), a.eval("ENGINE"), a.writes());
        let bridge: mlua::Table = a
            .lua
            .globals()
            .get::<mlua::Table>("ExposedMembers")
            .unwrap()
            .get("JAC_RP_HeroUI")
            .unwrap();
        let send: Function = bridge.get("ResolveDiscovery").unwrap();
        assert!(send
            .call::<bool>((R1, "d", outcome, json_to_lua(&a.lua, &a.options(json!({})))))
            .unwrap());
        assert_eq!(
            (a.snapshot(), a.eval("ENGINE"), a.writes()),
            before,
            "{outcome}: UI must not apply the effect"
        );
        assert_eq!(a.eval("#REQUESTS"), 1);
        assert_eq!(a.eval("REQUESTS[1].actor"), 1);
        assert_eq!(
            a.eval("REQUESTS[1].params.OnStart"),
            "JAC_ReligionOperation"
        );
        let packet = a.eval("REQUESTS[1].params");
        for h in [&a, &b] {
            let receive: Function = h.lua.globals().get("RELIGION_OPERATION").unwrap();
            assert!(
                receive.call::<bool>((1, packet.to_lua(&h.lua))).unwrap(),
                "{outcome}"
            );
            let applied = (h.snapshot(), h.eval("ENGINE"), h.writes());
            assert!(!receive.call::<bool>((1, packet.to_lua(&h.lua))).unwrap());
            assert_eq!((h.snapshot(), h.eval("ENGINE"), h.writes()), applied);
            assert_eq!(h.s()["discoveries"]["d"]["effects"], "APPLIED");
        }
        assert_eq!(a.snapshot(), b.snapshot(), "{outcome}");
        assert_eq!(a.eval("ENGINE"), b.eval("ENGINE"), "{outcome}");
    }
}

#[test]
fn multiplayer_direct_revival_requires_material_and_city_owned_by_engine_actor() {
    let mut a = Harness::new();
    a.f();
    a.l();
    call!(a, "PreserveAsset", R1, "book", "SCRIPTURE", "PLAYER_1", 1);
    let mut b = Harness::new();
    b.load(Some(a.export()), None, Some(a.eval("ENGINE")))
        .unwrap();
    let request = network_request(
        &a,
        "REVIVE",
        json!({"Revival":"r",
        "Community":"new","Source":"book","City":0}),
    );
    for h in [&a, &b] {
        h.exec("MULTIPLAYER=true");
        assert_network_unchanged(h, 0, &request);
        let mut invalid = request.clone();
        invalid["City"] = json!(999);
        assert_network_unchanged(h, 1, &invalid);
        assert!(network_event(h, 1, &request));
        assert_eq!(h.engine("1")["faith"], 480);
    }
    for h in [&mut a, &mut b] {
        h.reload();
        h.exec("MULTIPLAYER=true");
        assert_network_unchanged(h, 1, &request);
    }
    assert_eq!(a.snapshot(), b.snapshot());
    assert_eq!(a.eval("ENGINE"), b.eval("ENGINE"));
}

#[test]
fn multiplayer_saved_turn_authority_survives_load_and_expires_on_end_or_next_turn() {
    let mut h = network_discovery();
    h.exec("RELIGION_TURN_END(0)");
    h.reload();
    h.exec("MULTIPLAYER=true");
    let request = network_request(&h, "RESOLVE", json!({"Outcome":"SELL"}));
    assert_network_unchanged(&h, 0, &request);
    h.exec("TURN=TURN+1");
    let request = network_request(&h, "RESOLVE", json!({"Outcome":"SELL"}));
    assert_network_unchanged(&h, 1, &request);
    h.exec("RELIGION_TURN_START(1)");
    assert!(network_event(&h, 1, &request));
}

#[test]
fn multiplayer_native_hero_requires_actor_proposal_before_reading_ui_replica() {
    let h = network_discovery();
    h.exec("ExposedMembers.JAC_RP_HeroUI.OriginBuilding=function() error('UI read') end; ExposedMembers.JAC_RP_HeroUI.ClaimPlayer=function() error('UI read') end");
    let request = network_request(&h, "RESOLVE", json!({"Outcome":"HERO","City":0}));
    assert_network_unchanged(&h, 1, &request);
    assert!(h.truth("string.find(LOG[#LOG], 'requires an actor proposal',1,true)~=nil"));
    assert_eq!(h.eval("#REQUESTS"), 0);
    assert!(manifest_text().contains("<SupportsMultiplayer>0</SupportsMultiplayer>"));
}

#[test]
fn multiplayer_hero_owner_observation_completes_on_two_clients_without_receiver_ui_reads() {
    let mut a = network_discovery();
    let mut b = network_discovery();
    a.exec("DEFER_OPERATIONS=true; LOCAL_PLAYER=1; assert(ExposedMembers.JAC_RP_HeroUI.ResolveDiscovery('JAC_RP_PLACEHOLDER_01','d','HERO',{city=0}))");
    b.exec("LOCAL_PLAYER=0; ExposedMembers.JAC_RP_HeroUI.OriginBuilding=function() error('foreign UI') end; ExposedMembers.JAC_RP_HeroUI.ClaimPlayer=function() error('foreign UI') end; ExposedMembers.JAC_RP_HeroUI.UnitInfo=function() error('foreign UI') end");
    let proposal = a.eval("REQUESTS[1].params");
    for h in [&mut a, &mut b] {
        let receive: Function = h.lua.globals().get("RELIGION_OPERATION").unwrap();
        assert!(receive.call::<bool>((1, proposal.to_lua(&h.lua))).unwrap());
        assert_eq!(hero_verification(h), "PENDING");
        assert_eq!(h.engine("1")["faith"], 500);
        assert_eq!(h.export()["heroes"]["1"]["city"], 0);
        assert_eq!(h.export()["heroes"]["1"]["player"], 1);
        assert!(!receive.call::<bool>((1, proposal.to_lua(&h.lua))).unwrap());
        h.reload();
        h.exec("MULTIPLAYER=true");
    }
    assert_eq!(a.snapshot(), b.snapshot());
    assert_eq!(a.eval("ENGINE"), b.eval("ENGINE"));
    a.exec("LOCAL_PLAYER=1; DEFER_OPERATIONS=true; HERO_UI_TICK(0); HERO_UI_TICK(0)");
    assert_eq!(a.eval("#REQUESTS"), 1);
    assert_eq!(a.engine("1")["faith"], 500);
    let observation = a.eval("REQUESTS[1].params");
    for h in [&mut a, &mut b] {
        h.exec("ExposedMembers.JAC_RP_HeroUI.UnitInfo=function() error('receiver UI read') end; ExposedMembers.JAC_RP_HeroUI.ClaimPlayer=function() error('receiver UI read') end");
        let receive: Function = h.lua.globals().get("RELIGION_OPERATION").unwrap();
        assert!(!receive
            .call::<bool>((0, observation.to_lua(&h.lua)))
            .unwrap());
        assert!(receive
            .call::<bool>((1, observation.to_lua(&h.lua)))
            .unwrap());
        assert_eq!(hero_verification(h), "APPLIED");
        assert_eq!(h.engine("1")["faith"], 532.5);
        assert_eq!(
            h.export()["heroes"]["1"]["claimIndependentlyVerified"],
            false
        );
        let before = (h.snapshot(), h.eval("ENGINE"));
        assert!(!receive
            .call::<bool>((1, observation.to_lua(&h.lua)))
            .unwrap());
        assert_eq!((h.snapshot(), h.eval("ENGINE")), before);
        h.reload();
        h.exec("MULTIPLAYER=true");
        let receive: Function = h.lua.globals().get("RELIGION_OPERATION").unwrap();
        assert!(!receive
            .call::<bool>((1, observation.to_lua(&h.lua)))
            .unwrap());
        assert_eq!(h.engine("1")["faith"], 532.5);
    }
    assert_eq!(a.snapshot(), b.snapshot());
    assert_eq!(a.eval("ENGINE"), b.eval("ENGINE"));
}

fn hero_observation_request(h: &Harness) -> Json {
    let hero = &h.export()["heroes"]["1"];
    network_request(
        h,
        "COMPLETE_HERO",
        json!({"Unit":hero["unit"].python_f64().unwrap() as u64,
        "City":0,"ReservedTurn":17,"Token":hero["token"].as_str().unwrap(),
        "ObservedClass":10,"ObservedClaim":1,"ObservedLifespan":30}),
    )
}

#[test]
fn multiplayer_hero_observation_rejects_invalid_fields_and_wrong_core_unit_before_payment() {
    let h = network_discovery();
    let proposal = network_request(
        &h,
        "RESOLVE",
        json!({"Outcome":"HERO","City":0,
        "HeroClass":10,"HeroClaim":-1}),
    );
    assert!(network_event(&h, 1, &proposal));
    let request = hero_observation_request(&h);
    for (key, value) in [
        ("Unit", json!(2)),
        ("City", json!(1)),
        ("ReservedTurn", json!(16)),
        ("Token", json!("old")),
        ("ObservedClass", json!(11)),
        ("ObservedClaim", json!(0)),
        ("ObservedLifespan", json!(0)),
        ("ObservedLifespan", json!(1.5)),
        ("ObservedLifespan", json!("30")),
        ("Revision", json!(0)),
    ] {
        let mut invalid = request.clone();
        invalid[key] = value;
        assert_network_unchanged(&h, 1, &invalid);
    }
    for core_change in [
        "ENGINE['1'].units['1'].kind=5",
        "ENGINE['1'].units['1'].owner=0",
        "ENGINE['1'].units['1']=nil",
    ] {
        let bad = network_discovery();
        assert!(network_event(&bad, 1, &proposal));
        bad.exec(core_change);
        assert_network_unchanged(&bad, 1, &request);
    }
    h.exec("RELIGION_TURN_END(1)");
    assert_network_unchanged(&h, 1, &request);
    h.exec("RELIGION_TURN_START(1); ExposedMembers.JAC_RP_HeroUI.UnitInfo=function() error('Gameplay UI read') end");
    assert!(network_event(&h, 1, &request));
    assert_eq!(h.engine("1")["faith"], 532.5);
}

#[test]
fn multiplayer_hero_proposal_rejects_foreign_source_city_and_duplicate_class() {
    let h = network_discovery();
    let request = network_request(
        &h,
        "RESOLVE",
        json!({"Outcome":"HERO","City":0,
        "HeroClass":10,"HeroClaim":-1}),
    );
    assert_network_unchanged(&h, 0, &request);
    for (key, value) in [
        ("City", json!(99)),
        ("HeroClass", json!(999)),
        ("HeroClaim", json!(0)),
    ] {
        let mut invalid = request.clone();
        invalid[key] = value;
        assert_network_unchanged(&h, 1, &invalid);
    }
    h.exec("ENGINE['0'].units['80']={id=80,kind=100,class=10,lifespan=30}");
    assert_network_unchanged(&h, 1, &request);
    h.exec("ENGINE['0'].units['80']=nil; ENGINE['1'].cities['0'].monument=false");
    assert_network_unchanged(&h, 1, &request);
    h.exec("ENGINE['1'].cities['0'].monument=true");
    assert!(network_event(&h, 1, &request));
}

#[test]
fn multiplayer_hero_interrupted_reward_never_replays_after_saved_resubmission() {
    let mut h = network_discovery();
    let proposal = network_request(
        &h,
        "RESOLVE",
        json!({"Outcome":"HERO","City":0,
        "HeroClass":10,"HeroClaim":-1}),
    );
    assert!(network_event(&h, 1, &proposal));
    let request = hero_observation_request(&h);
    h.exec("FAIL_WRITE_AT=WRITES+2");
    assert!(!network_event(&h, 1, &request));
    assert_eq!(hero_verification(&h), "APPLYING");
    assert_eq!(h.engine("1")["faith"], 532.5);
    h.reload();
    h.exec("MULTIPLAYER=true");
    assert_network_unchanged(&h, 1, &request);
    let fresh = hero_observation_request(&h);
    assert_network_unchanged(&h, 1, &fresh);
}

#[test]
fn synchronized_singleplayer_hero_completion_waits_for_publish_and_pays_once_after_load() {
    let mut h = Harness::new();
    pending_hero(&h);
    h.exec("DEFER_OPERATIONS=true; HERO_UI_TICK(0); HERO_UI_TICK(0)");
    assert_eq!(
        h.eval("#REQUESTS"),
        1,
        "queued requests must not flood frames"
    );
    assert_eq!(hero_verification(&h), "PENDING");
    let params = h.eval("REQUESTS[1].params");
    h.lua
        .globals()
        .set("REPLAY", params.to_lua(&h.lua))
        .unwrap();
    h.exec("assert(RELIGION_OPERATION(1,REPLAY)); assert(not RELIGION_OPERATION(1,REPLAY))");
    assert_eq!(h.engine("1")["faith"], 532.5);
    h.reload();
    h.lua
        .globals()
        .set("REPLAY", params.to_lua(&h.lua))
        .unwrap();
    h.exec("assert(not RELIGION_OPERATION(1,REPLAY)); HERO_UI_TICK(0)");
    assert_eq!(h.engine("1")["faith"], 532.5);
}

#[test]
fn synchronized_hero_checks_resume_for_receipt_owner_after_local_player_change() {
    let h = Harness::new();
    pending_hero(&h);
    h.exec("LOCAL_PLAYER=0; HERO_UI_TICK(0)");
    assert_eq!(h.eval("#REQUESTS"), 0);
    assert_eq!(hero_verification(&h), "PENDING");
    h.exec("LOCAL_PLAYER=1; HERO_UI_PUBLISH(); HERO_UI_TICK(0)");
    assert_eq!(h.eval("#REQUESTS"), 1);
    assert_eq!(hero_verification(&h), "APPLIED");
    assert_eq!(h.engine("1")["faith"], 532.5);
}

#[test]
fn singleplayer_ai_hero_completes_on_engine_turn_event_without_foreign_ui_request() {
    let mut h = Harness::new();
    pending_hero(&h);
    h.exec("HUMAN_ONE=false; LOCAL_PLAYER=0; HERO_UI_TICK(0)");
    assert_eq!(h.eval("#REQUESTS"), 0);
    assert_eq!(hero_verification(&h), "PENDING");
    h.exec("RELIGION_TURN_END(1)");
    assert_eq!(hero_verification(&h), "APPLIED");
    assert_eq!(h.engine("1")["faith"], 532.5);
    h.reload();
    h.exec("HUMAN_ONE=false; LOCAL_PLAYER=0; RELIGION_TURN_START(1); RELIGION_TURN_END(1)");
    assert_eq!(h.engine("1")["faith"], 532.5);
    assert_eq!(h.eval("#REQUESTS"), 0);
}

#[test]
fn multiplayer_interrupted_engine_effect_stays_reserved_across_saved_retransmission() {
    for fail_after_payment in [false, true] {
        let mut h = network_discovery();
        let request = network_request(&h, "RESOLVE", json!({"Outcome":"SELL"}));
        if fail_after_payment {
            h.exec("FAIL_WRITE_AT=WRITES+2");
        } else {
            h.exec("FAIL_EFFECT=true");
        }
        assert!(!network_event(&h, 1, &request));
        assert_eq!(h.s()["discoveries"]["d"]["effects"], "PENDING");
        assert_eq!(
            h.engine("1")["gold"],
            if fail_after_payment { 600 } else { 100 }
        );
        h.reload();
        h.exec("MULTIPLAYER=true");
        assert_network_unchanged(&h, 1, &request);
        let fresh = network_request(&h, "RESOLVE", json!({"Outcome":"SELL"}));
        assert_network_unchanged(&h, 1, &fresh);
    }
}

#[test]
fn test_timed_out_hero_completes_once_after_engine_publishes_valid_lifespan() {
    let h = Harness::new();
    pending_hero(&h);
    h.exec("local x=rp.Export().heroes['1']; ENGINE['1'].units[tostring(x.unit)].lifespan=nil; HERO_UI_TICK(31); HERO_UI_PUBLISH()");
    assert_eq!(hero_verification(&h), "PENDING");
    assert_eq!(h.engine("1")["faith"], 500);
    let diag = h.eval("ExposedMembers.JAC_RP_HeroUI.VerificationDiagnostics(1)");
    assert!(diag["timedOut"].python_truthy());
    assert_eq!(
        (
            diag["elapsed"].clone(),
            diag["frameChecks"].clone(),
            diag["publishedChecks"].clone()
        ),
        (json!(31).into(), json!(0).into(), json!(1).into())
    );
    h.exec("HERO_UI_TICK(1)");
    let after = h.eval("ExposedMembers.JAC_RP_HeroUI.VerificationDiagnostics(1)");
    assert_eq!(
        (
            after["elapsed"].clone(),
            after["frameChecks"].clone(),
            after["publishedChecks"].clone()
        ),
        (json!(31).into(), json!(0).into(), json!(1).into())
    );
    h.exec("local x=rp.Export().heroes['1']; ENGINE['1'].units[tostring(x.unit)].lifespan=30; ENGINE['1'].active=false; HERO_UI_PUBLISH()");
    assert_eq!(hero_verification(&h), "PENDING");
    assert_eq!(h.engine("1")["faith"], 500);
    h.exec("ENGINE['1'].active=true; HERO_UI_PUBLISH()");
    assert_eq!(hero_verification(&h), "APPLIED");
    assert_eq!(h.engine("1")["faith"], 532.5);
    h.exec("HERO_UI_PUBLISH(); HERO_UI_TICK(1)");
    assert_eq!(h.engine("1")["faith"], 532.5);
    let completed = h.eval("ExposedMembers.JAC_RP_HeroUI.VerificationDiagnostics(1)");
    assert!(!completed["timedOut"].python_truthy());
    assert_eq!(
        (
            completed["frameChecks"].clone(),
            completed["publishedChecks"].clone()
        ),
        (json!(0).into(), json!(2).into())
    );
}

#[test]
fn test_hero_waits_for_valid_class_without_querying_negative_claim() {
    let h = Harness::new();
    pending_hero(&h);
    h.exec("local x=rp.Export().heroes['1']; ENGINE['1'].units[tostring(x.unit)].class=nil; HERO_UI_TICK()");
    assert_eq!(hero_verification(&h), "PENDING");
    assert_eq!(h.engine("1")["faith"], 500);
    h.exec("local x=rp.Export().heroes['1']; ENGINE['1'].units[tostring(x.unit)].class=10; HERO_UI_TICK()");
    assert_eq!(hero_verification(&h), "APPLIED");
    assert_eq!(h.engine("1")["faith"], 532.5);
}

#[test]
fn test_hero_waits_for_replicated_lifespan_property_before_reward() {
    let h = Harness::new();
    pending_hero(&h);
    h.exec("local x=rp.Export().heroes['1']; ENGINE['1'].units[tostring(x.unit)].lifespan=nil; HERO_UI_TICK()");
    assert_eq!(hero_verification(&h), "PENDING");
    assert_eq!(h.engine("1")["faith"], 500);
    h.exec("local x=rp.Export().heroes['1']; ENGINE['1'].units[tostring(x.unit)].lifespan=30; HERO_UI_TICK()");
    assert_eq!(hero_verification(&h), "APPLIED");
    assert_eq!(h.engine("1")["faith"], 532.5);
}

#[test]
fn test_hero_verification_accepts_unset_test_flag_in_manual_game() {
    for getter in ["function() return nil end", "function() end"] {
        let mut h = Harness::new();
        pending_hero(&h);
        h.reload();
        h.exec(&format!(
            "GameConfiguration.GetValue={getter}; HERO_UI_LOAD_CLOSE(); HERO_UI_FRAME(0)"
        ));
        assert_eq!(hero_verification(&h), "APPLIED");
        assert_eq!(h.engine("1")["faith"], 532.5);
    }
}

#[test]
fn test_hidden_addin_activates_after_load_and_completes_pending_hero_once() {
    let mut h = Harness::new();
    pending_hero(&h);
    h.reload();
    let before = h.engine("1")["faith"].python_f64().unwrap();
    h.exec("HERO_UI_FRAME(0)");
    assert_eq!(hero_verification(&h), "PENDING");
    assert_eq!(h.engine("1")["faith"], before);
    h.exec("HERO_UI_LOAD_CLOSE(); HERO_UI_FRAME(0)");
    assert!(!h.truth("HERO_UI_HIDDEN"));
    assert_eq!(hero_verification(&h), "APPLIED");
    assert_eq!(h.engine("1")["faith"], before + 32.5);
    h.exec("HERO_UI_LOAD_CLOSE(); HERO_UI_FRAME(0)");
    assert_eq!(h.engine("1")["faith"], before + 32.5);
}

#[test]
fn test_official_hero_requires_mode_origin_building_and_unclaimed_hero() {
    let mut h = Harness::new();
    h.f();
    h.l();
    h.d();
    h.exec("HEROES_MODE=false");
    h.rejected(
        "ResolveDiscovery",
        vec![json!(R1), json!("d"), json!("HERO"), h.options(json!({}))],
    );
    h.exec("HEROES_MODE=true; ENGINE['1'].cities['0'].monument=false");
    h.rejected(
        "ResolveDiscovery",
        vec![json!(R1), json!("d"), json!("HERO"), h.options(json!({}))],
    );
    h.exec("ENGINE['1'].cities['0'].monument=true; ENGINE['0'].heroClaims['10']=0; ENGINE['0'].heroClaims['11']=0");
    h.rejected(
        "ResolveDiscovery",
        vec![json!(R1), json!("d"), json!("HERO"), h.options(json!({}))],
    );
    h.exec("ENGINE['0'].heroClaims['11']=nil; ENGINE['1'].faith=0");
    h.resolve(R1, "d", "HERO", json!({}));
    h.exec("HERO_UI_TICK()");
    assert_eq!(h.engine("1")["faith"], 32.5);
    let unit=h.eval_chunk("local receipt=rp.Export().heroes['1']; local units=Players[1]:GetUnits(); local u=units:FindID(receipt.unit); return {u:GetType(),u:GetHeroClassType(),u:GetProperty(UnitPropertyKeys.LifespanRemaining)}");
    assert_eq!(unit, json!([101, 11, 30]));
    assert_eq!(h.engine("0")["heroClaims"]["11"], 1);
    h.reload();
}

#[test]
fn test_hero_reward_waits_for_ui_verification_and_survives_pending_save() {
    let mut h = Harness::new();
    h.f();
    h.l();
    h.d();
    let before = h.engine("1")["faith"].python_f64().unwrap();
    h.resolve(R1, "d", "HERO", json!({}));
    assert_eq!(h.s()["discoveries"]["d"]["effects"], "PENDING");
    assert_eq!(h.engine("1")["faith"], before);
    h.reload();
    h.exec("HERO_UI_TICK()");
    assert_eq!(h.s()["discoveries"]["d"]["effects"], "APPLIED");
    assert_eq!(h.engine("1")["faith"], before + 32.5);
    h.exec("HERO_UI_TICK()");
    assert_eq!(h.engine("1")["faith"], before + 32.5);
}

#[test]
fn test_unverified_hero_claim_never_pays_faith() {
    let h = Harness::new();
    h.f();
    h.l();
    h.d();
    let before = h.engine("1")["faith"].python_f64().unwrap();
    h.resolve(R1, "d", "HERO", json!({}));
    h.exec("ENGINE['0'].heroClaims['10']=0");
    assert!(!h.call_truth("CompleteHero", vec![json!(1)]));
    assert_eq!(h.engine("1")["faith"], before);
    assert_eq!(h.s()["discoveries"]["d"]["effects"], "PENDING");
    h.exec("ENGINE['0'].heroClaims['10']=1; HERO_UI_TICK()");
    assert_eq!(h.engine("1")["faith"], before + 32.5);
}

#[test]
fn test_hero_reward_waits_until_recipients_turn() {
    let h = Harness::new();
    h.f();
    h.l();
    h.d();
    let before = h.engine("1")["faith"].python_f64().unwrap();
    h.resolve(R1, "d", "HERO", json!({}));
    h.exec("ENGINE['1'].active=false; HERO_UI_TICK(60)");
    assert_eq!(h.engine("1")["faith"], before);
    h.exec("ENGINE['1'].active=true; HERO_UI_TICK()");
    assert_eq!(h.engine("1")["faith"], before + 32.5);
}

#[test]
fn test_hero_timeout_does_not_expire_another_players_inactive_reservation() {
    let h = Harness::new();
    h.f();
    h.l();
    h.discovery("MONK", "MONK_1", R1, "book", "d", 0);
    h.found(R2, 3, 1, 0);
    h.lost(R2);
    h.discovery("MONK", "MONK_1", R2, "book2", "d2", 1);
    h.resolve(R1, "d", "HERO", json!({"player":0}));
    h.resolve(R2, "d2", "HERO", json!({"player":1}));
    let before = h.engine("1")["faith"].python_f64().unwrap();
    h.exec("ENGINE['1'].active=false; ENGINE['0'].heroClaims['10']=-1; HERO_UI_TICK(31)");
    assert_eq!(h.export()["heroes"]["1"]["verification"], "PENDING");
    h.exec("ENGINE['1'].active=true; HERO_UI_TICK(0)");
    assert_eq!(h.engine("1")["faith"], before + 32.5);
    assert_eq!(h.export()["heroes"]["1"]["verification"], "APPLIED");
}

#[test]
fn test_interrupted_hero_reward_cannot_replay_after_load() {
    let mut h = Harness::new();
    h.f();
    h.l();
    h.d();
    let before = h.engine("1")["faith"].clone();
    h.resolve(R1, "d", "HERO", json!({}));
    h.exec("FAIL_EFFECT=true");
    assert!(h.raw_call("CompleteHero", vec![json!(1)]).is_err());
    assert_eq!(hero_verification(&h), "APPLYING");
    h.reload();
    h.exec("HERO_UI_TICK()");
    assert_eq!(h.engine("1")["faith"], before);
    reject!(h, "CompleteHero", 1);
}

#[test]
fn test_invalid_inputs_and_world_overflow_do_not_mutate_saved_state() {
    let h = Harness::new();
    h.f();
    for value in [
        json!(-1),
        json!(0.5),
        json!(2147483648_i64),
        json!("1"),
        json!(true),
        Json::Null,
    ] {
        h.rejected(
            "SetCommunity",
            vec![
                json!(R1),
                json!("origin"),
                value,
                json!(0),
                json!("PLAYER_0"),
            ],
        );
    }
    for value in ["0/0", "math.huge"] {
        let before = h.snapshot();
        let writes = h.writes();
        assert!(h
            .lua
            .load(format!(
                "rp.SetCommunity('{R1}','origin',{value},0,'PLAYER_0')"
            ))
            .exec()
            .is_err());
        assert_eq!(h.snapshot(), before);
        assert_eq!(h.writes(), writes);
    }
    reject!(h, "SetCommunity", R1, "origin", 1, -1, "PLAYER_0");
    reject!(h, "SetCommunity", R1, "", 1, 0, "PLAYER_0");
    reject!(h, "SetCommunity", R1, "origin", 1, 0, "");
    reject!(h, "Found", R1, 0, "origin", 1, 0, "PLAYER_0");
    reject!(h, "Found", "UNKNOWN", 0, "origin", 1, 0, "PLAYER_0");
    reject!(h, "SetCommunity", R2, "origin", 1, 0, "PLAYER_0");
    call!(
        h,
        "SetCommunity",
        R1,
        "origin",
        2147483647_i64,
        0,
        "PLAYER_0"
    );
    reject!(h, "SetCommunity", R1, "elsewhere", 1, 0, "PLAYER_1");
}

#[test]
fn test_named_rules_and_all_eighty_modifier_definitions_match_official_types() {
    let h = Harness::new();
    assert_eq!(
        call!(h, "Rules"),
        json!({"REVIVAL_FOLLOWERS":1,"REVIVAL_FAITH_COST":20,"REVIVAL_COOLDOWN_TURNS":10,"SELL_GOLD":500,"RESEARCH_SCIENCE":10,"RESEARCH_CULTURE":10,"RESEARCH_EUREKAS":2,"HERESY_CULTURE":20,"HERESY_FAITH_COST":400,"ABSORB_FAITH_COST":500,"ABSORB_DIVISOR":3,"ARTIFACT_CULTURE":30,"HERO_FAITH_MULTIPLIER":10})
    );
    assert_eq!(h.tables["modifiers"].as_object().unwrap().len(), 80);
    let types: BTreeSet<_> = h.tables["modifierTypes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| {
            (
                r["ModifierType"].as_str().unwrap(),
                r["RunOnce"].as_i64().unwrap(),
                r["Permanent"].as_i64().unwrap(),
            )
        })
        .collect();
    assert_eq!(
        types,
        BTreeSet::from([("MODIFIER_PLAYER_GRANT_YIELD", 1, 1)])
    );
    for religion in h.tables["Religions"].as_array().unwrap() {
        for (outcome, yield_name, amount) in [
            ("RESEARCH", "SCIENCE", "10"),
            ("RESEARCH_CULTURE", "CULTURE", "10"),
            ("HERESY", "CULTURE", "20"),
            ("ARTIFACT", "CULTURE", "30"),
        ] {
            let id = format!("JAC_RP_{outcome}_{}", religion["ID"].as_str().unwrap());
            assert_eq!(
                h.tables["modifiers"][&id],
                json!({"YieldType":format!("YIELD_{yield_name}"),"Amount":amount,"Scale":"false"})
            );
        }
    }
    h.exec("local rules=rp.Rules(); rules.REVIVAL_FOLLOWERS=99");
    assert_eq!(h.eval("rp.Rules().REVIVAL_FOLLOWERS"), 1);
}

#[test]
fn test_revival_requires_faith_city_ownership_and_fixed_size() {
    let h = Harness::new();
    h.f();
    h.l();
    h.d();
    h.exec("ENGINE['1'].faith=19");
    h.rejected(
        "ResolveDiscovery",
        vec![json!(R1), json!("d"), json!("REVIVE"), h.options(json!({}))],
    );
    h.exec("ENGINE['1'].faith=20");
    for extra in [
        json!({"city":99}),
        json!({"player":0}),
        json!({"followers":99}),
    ] {
        h.rejected(
            "ResolveDiscovery",
            vec![json!(R1), json!("d"), json!("REVIVE"), h.options(extra)],
        );
    }
    h.resolve(R1, "d", "REVIVE", json!({}));
    assert_eq!(h.engine("1")["faith"], 0);
    assert_eq!(
        h.s()["communities"]["new"],
        json!({"followers":1,"publicFacilities":0,"holder":"PLAYER_1"})
    );
}

#[test]
fn test_conceal_has_no_yield_and_keeps_material_after_later_paid_revivals() {
    let mut h = Harness::new();
    h.f();
    h.l();
    h.d();
    let engine = h.eval("ENGINE");
    h.resolve(R1, "d", "CONCEAL", json!({}));
    assert_eq!(h.eval("ENGINE"), engine);
    assert_eq!(h.s()["assets"]["book"]["holder"], "HIDDEN_PLAYER_1");
    assert!(h.s()["canRevive"].python_truthy());
    h.reload();
    call!(h, "Revive", R1, "r", "book", "new", 1, 0);
    call!(h, "SetCommunity", R1, "new", 0, 0, "PLAYER_1");
    assert!(h.s()["canRevive"].python_truthy());
    h.exec("TURN=TURN+10");
    call!(h, "Revive", R1, "r2", "book", "new2", 1, 0);
    assert_eq!(h.engine("1")["faith"], 460);
}

#[test]
fn test_reused_material_removed_ids_and_moved_discoveries_cannot_pay_rewards() {
    let mut h = Harness::new();
    h.f();
    h.l();
    h.d();
    call!(h, "MoveAsset", R1, "book", "CAVE", 1);
    h.rejected(
        "ResolveDiscovery",
        vec![json!(R1), json!("d"), json!("SELL"), h.options(json!({}))],
    );
    reject!(
        h,
        "Discover",
        R1,
        "d2",
        "book",
        "SCRIPTURE",
        "ACTOR",
        "THIEF",
        "CAVE",
        1
    );
    call!(h, "MoveAsset", R1, "book", "MONK_1", 1);
    h.resolve(R1, "d", "SELL", json!({}));
    call!(h, "RemoveAsset", R1, "book");
    reject!(h, "PreserveAsset", R1, "book", "SCRIPTURE", "CAVE");
    reject!(
        h,
        "Discover",
        R1,
        "d3",
        "book",
        "SCRIPTURE",
        "ACTOR",
        "THIEF",
        "CAVE",
        1
    );
    h.reload();
    assert_eq!(h.engine("1")["gold"], 600);
}

#[test]
fn test_non_revival_limit_is_per_player_religion_and_branch() {
    let h = Harness::new();
    h.f();
    h.l();
    h.d();
    h.resolve(R1, "d", "SELL", json!({}));
    h.discovery("MONK", "MONK_1", R1, "book2", "d2", 1);
    h.rejected(
        "ResolveDiscovery",
        vec![json!(R1), json!("d2"), json!("SELL"), h.options(json!({}))],
    );
    h.resolve(R1, "d2", "RESEARCH", json!({}));
    h.discovery("MONK", "MONK_1", R1, "book3", "d3", 0);
    h.resolve(R1, "d3", "SELL", json!({}));
    h.found(R2, 3, 1, 0);
    h.lost(R2);
    h.discovery("MONK", "MONK_1", R2, "other", "other", 1);
    h.resolve(R2, "other", "SELL", json!({}));
    assert_eq!(
        (
            h.engine("0")["gold"].clone(),
            h.engine("1")["gold"].clone(),
            h.engine("1")["science"].clone()
        ),
        (json!(600).into(), json!(1100).into(), json!(10).into())
    );
}

#[test]
fn test_absorption_requires_own_living_target_and_locks_the_transferred_source() {
    let mut h = Harness::new();
    h.f();
    h.l();
    h.d();
    h.rejected(
        "ResolveDiscovery",
        vec![
            json!(R1),
            json!("d"),
            json!("ABSORB"),
            h.options(json!({"targetReligion":R1})),
        ],
    );
    h.found(R2, 3, 1, 0);
    h.rejected(
        "ResolveDiscovery",
        vec![json!(R1), json!("d"), json!("ABSORB"), h.options(json!({}))],
    );
    call!(h, "SetCommunity", R2, "origin", 3, 1, "PLAYER_1");
    h.bind_absorption();
    h.resolve(R1, "d", "ABSORB", json!({}));
    assert_eq!(call!(h, "NativeFollowers", R1), 4);
    assert_eq!(call!(h, "NativeFollowers", R2), 7);
    assert_eq!(h.engine("1")["faith"], 0);
    h.lost(R2);
    assert!(!h.state(R2)["canRevive"].python_truthy());
    reject!(h, "Revive", R2, "newrevival", "book", "new", 1, 0);
    h.reload();
    assert_eq!(h.state(R2)["materialUses"]["book"]["outcome"], "ABSORB");
}

#[test]
fn test_diplomatic_restitution_requires_foreign_founder_contact_and_peace() {
    let h = Harness::new();
    h.f();
    h.l();
    h.d();
    h.rejected(
        "ResolveDiscovery",
        vec![
            json!(R1),
            json!("d"),
            json!("DIPLOMATIC_INCIDENT"),
            h.options(json!({"targetPlayer":1})),
        ],
    );
    h.exec("ENGINE['1'].met=false");
    h.rejected(
        "ResolveDiscovery",
        vec![
            json!(R1),
            json!("d"),
            json!("DIPLOMATIC_INCIDENT"),
            h.options(json!({})),
        ],
    );
    h.exec("ENGINE['1'].met=true; ENGINE['1'].war=true");
    h.rejected(
        "ResolveDiscovery",
        vec![
            json!(R1),
            json!("d"),
            json!("DIPLOMATIC_INCIDENT"),
            h.options(json!({})),
        ],
    );
    h.exec("ENGINE['1'].war=false; ENGINE['0'].alive=false");
    h.rejected(
        "ResolveDiscovery",
        vec![
            json!(R1),
            json!("d"),
            json!("DIPLOMATIC_INCIDENT"),
            h.options(json!({})),
        ],
    );
    h.exec("ENGINE['0'].alive=true");
    h.resolve(R1, "d", "DIPLOMATIC_INCIDENT", json!({}));
    assert_eq!(
        (
            h.engine("1")["favor"].clone(),
            h.s()["assets"]["book"]["holder"].clone()
        ),
        (json!(15).into(), json!("PLAYER_0").into())
    );
}

#[test]
fn test_vault_and_hero_limits_apply_across_religions_after_load() {
    for outcome in ["ARTIFACT", "HERO"] {
        let mut h = Harness::new();
        for religion in [R1, R2] {
            h.found(religion, 3, 1, 0);
            h.lost(religion);
            h.discovery("MONK", "MONK_1", religion, "book", "d", 1);
        }
        h.resolve(R1, "d", outcome, json!({}));
        h.reload();
        h.rejected(
            "ResolveDiscovery",
            vec![json!(R2), json!("d"), json!(outcome), h.options(json!({}))],
        );
    }
}

#[test]
fn test_engine_refusal_and_missing_spawn_leave_pending_and_block_replay() {
    for (failure, outcome) in [("FAIL_EFFECT", "SELL"), ("FAIL_SPAWN", "HERO")] {
        let mut h = Harness::new();
        h.f();
        h.l();
        h.d();
        h.exec(&format!("{failure}=true"));
        assert!(h
            .raw_call(
                "ResolveDiscovery",
                vec![json!(R1), json!("d"), json!(outcome), h.options(json!({}))]
            )
            .is_err());
        assert_eq!(h.s()["discoveries"]["d"]["effects"], "PENDING");
        if failure == "FAIL_SPAWN" {
            assert_eq!(h.engine("1")["faith"], 500);
            assert_eq!(h.engine("1")["units"], json!({}));
        }
        h.reload();
        h.rejected(
            "ResolveDiscovery",
            vec![json!(R1), json!("d"), json!(outcome), h.options(json!({}))],
        );
    }
}

#[test]
fn test_reservation_write_failure_has_no_engine_effect_and_receipt_failure_cannot_replay() {
    for offset in [1, 2] {
        let mut h = Harness::new();
        h.f();
        h.l();
        h.d();
        h.exec(&format!("FAIL_WRITE_AT=WRITES+{offset}"));
        assert!(h
            .raw_call(
                "ResolveDiscovery",
                vec![json!(R1), json!("d"), json!("SELL"), h.options(json!({}))]
            )
            .is_err());
        assert_eq!(h.engine("1")["gold"], if offset == 1 { 100 } else { 600 });
        if offset == 1 {
            assert!(h.s()["discoveries"]["d"].get("outcome").is_none());
        } else {
            assert_eq!(h.s()["discoveries"]["d"]["effects"], "PENDING");
            h.reload();
            h.rejected(
                "ResolveDiscovery",
                vec![json!(R1), json!("d"), json!("SELL"), h.options(json!({}))],
            );
        }
    }
}

#[test]
fn test_legacy_v1_choices_never_pay_on_load_or_unlock_used_material() {
    let mut h = Harness::new();
    h.f();
    h.l();
    h.d();
    let mut old = h.export();
    old.as_object_mut().unwrap().remove("heroes");
    old.as_object_mut().unwrap().remove("artifacts");
    let religion = &mut old["religions"][R1];
    for key in ["materialUses", "outcomeUses", "retiredAssets"] {
        religion.as_object_mut().unwrap().remove(key);
    }
    religion["discoveries"]["d"]["outcome"] = json!("SELL").into();
    religion["discoveries"]["d"]["effects"] = json!("DEFERRED").into();
    religion["discoveries"]["d"]["resolvedTurn"] = json!(17).into();
    h.load(Some(old), None, None).unwrap();
    assert_eq!(h.writes(), 0);
    assert_eq!(h.engine("1")["gold"], 100);
    assert!(!h.s()["canRevive"].python_truthy());
    h.rejected(
        "ResolveDiscovery",
        vec![json!(R1), json!("d"), json!("SELL"), h.options(json!({}))],
    );
}

#[test]
fn test_ai_prefers_each_job_branch_and_uses_legal_absorption_or_conceal_fallback() {
    for (job, expected) in [
        ("MONK", "REVIVE"),
        ("SCHOLAR", "RESEARCH"),
        ("THIEF", "SELL"),
        ("PERFORMER", "HERESY"),
        ("EXPLORER", "ARTIFACT"),
        ("KABUKIMONO", "HERO"),
        ("TRAVELER", "DIPLOMATIC_INCIDENT"),
    ] {
        let h = Harness::new();
        h.exec("HUMAN_ONE=false");
        h.f();
        h.l();
        h.discovery(job, "MONK_1", R1, "book", "d", 1);
        assert_eq!(
            h.call(
                "ChooseOutcome",
                vec![json!(R1), json!("d"), h.options(json!({}))]
            ),
            expected,
            "{job}"
        );
        assert_eq!(
            h.call(
                "ResolveForAI",
                vec![json!(R1), json!("d"), h.options(json!({}))]
            ),
            expected,
            "{job}"
        );
    }
    let h = Harness::new();
    h.exec("HUMAN_ONE=false");
    h.f();
    h.l();
    h.discovery("THIEF", "MONK_1", R1, "book", "d", 1);
    h.resolve(R1, "d", "SELL", json!({}));
    h.discovery("THIEF", "MONK_1", R1, "book2", "d2", 1);
    h.discovery("MONK", "MONK_1", R1, "book3", "d3", 1);
    h.found(R2, 3, 1, 0);
    call!(h, "SetCommunity", R2, "origin", 1, 0, "PLAYER_1");
    h.bind_absorption();
    assert_eq!(
        h.call(
            "ResolveForAI",
            vec![json!(R1), json!("d2"), h.options(json!({}))]
        ),
        "ABSORB"
    );
    h.exec("ENGINE['1'].faith=0; ENGINE['1'].met=false; ENGINE['1'].city=false");
    assert_eq!(
        h.call(
            "ResolveForAI",
            vec![json!(R1), json!("d3"), h.options(json!({}))]
        ),
        "CONCEAL"
    );
}

#[test]
fn test_engine_write_failure_and_export_edits_leave_original_untouched() {
    let h = Harness::new();
    h.f();
    let saved = h.snapshot();
    h.exec("local exported=rp.Export(); exported.religions['JAC_RP_PLACEHOLDER_01'].communities.origin.followers=99; local inspected=rp.Inspect('JAC_RP_PLACEHOLDER_01'); inspected.assets.injected={kind='RELIC',holder='CAVE'}");
    assert_eq!(h.snapshot(), saved);
    h.exec("FAIL_WRITE=true");
    reject!(h, "SetCommunity", R1, "origin", 0, 0, "PLAYER_0");
    h.exec("FAIL_WRITE=false");
    call!(h, "SetCommunity", R1, "origin", 1, 0, "PLAYER_0");
    assert_eq!(h.s()["followers"], 1);
}

#[test]
fn test_twenty_religions_do_not_leak_state_across_save_load() {
    let mut h = Harness::new();
    for n in 1..=20 {
        h.found(&format!("JAC_RP_PLACEHOLDER_{n:02}"), n, 1, n);
    }
    call!(h, "PreserveAsset", R1, "book", "SCRIPTURE", "CAVE");
    h.l();
    call!(h, "NotifyFounderDefeated", 1);
    h.reload();
    assert_eq!(h.export()["religions"].as_object().unwrap().len(), 20);
    assert!(h.s()["lost"].python_truthy());
    assert_eq!(
        (
            h.state(R2)["followers"].clone(),
            h.state(R2)["state"].clone()
        ),
        (json!(2).into(), json!("PUBLIC").into())
    );
    assert_eq!(h.state(R2)["assets"], json!({}));
}

#[test]
fn test_switching_save_in_same_vm_does_not_use_previous_game_cache() {
    let h = Harness::new();
    h.f();
    let first = h.export();
    h.found(R2, 3, 1, 0);
    let property_store: mlua::Table = h.lua.globals().get("PROPERTY_STORE").unwrap();
    property_store.set(KEY, first.to_lua(&h.lua)).unwrap();
    assert_eq!(
        h.export()["religions"]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([R1])
    );
    property_store.set(KEY, Value::Nil).unwrap();
    assert_eq!(h.export()["religions"], json!({}));
}

#[test]
fn test_future_or_damaged_save_fails_without_overwrite() {
    for saved in [
        json!({"schema":2,"revision":8,"religions":{}}),
        json!({"schema":1,"revision":8,"religions":"bad"}),
        json!("bad"),
    ] {
        let mut h = Harness::new();
        assert!(h.load(Some(saved.clone().into()), None, None).is_err());
        assert_eq!(h.writes(), 0);
        assert_eq!(h.eval("PROPERTY_STORE.JAC_RP_STATE_V1"), saved);
    }
}

fn scenario() -> Json {
    serde_json::from_str(&read_repo(
        "tools/civ6-autotest/scenarios/religion-persistence.json",
    ))
    .unwrap()
}

#[test]
fn test_live_scenario_gameplay_and_external_captures_in_fresh_vms() {
    let mut h = Harness::new();
    h.exec("LOCAL_PLAYER=0; HUMAN_ONE=false");
    let scenario = scenario();
    let mut captures: BTreeMap<String, Data> = BTreeMap::new();
    let mut saves: BTreeMap<String, (Data, Data, Data)> = BTreeMap::new();
    let (mut compared, mut loaded, mut native_founded) = (0, 0, 0);
    for (index, step) in scenario["steps"].as_array().unwrap().iter().enumerate() {
        let action = step["action"].as_str().unwrap();
        let code = step["code"].as_str().unwrap_or("");
        if code.starts_with("-- Keep hero verification pending") {
            h.exec(code);
        } else if action == "ui_lua" && step["state_name"] == "GameCore_Tuner" {
            let value = h.eval_chunk(code);
            h.exec("HERO_UI_TICK()");
            if value == "native placeholder religion founded" {
                native_founded += 1;
                assert_eq!(h.eval("NATIVE_RELIGION"), 42);
            }
            if let Some(name) = step["equals_capture"].as_str() {
                assert_eq!(
                    value,
                    *captures.get(name).unwrap(),
                    "scenario step {index} capture {name}"
                );
                compared += 1;
            }
            if let Some(name) = step["capture_as"].as_str() {
                captures.insert(name.to_owned(), value);
            }
        } else if action == "save" {
            saves.insert(
                step["name"].as_str().unwrap().to_owned(),
                (h.export(), h.eval("ENGINE"), h.eval("TURN")),
            );
        } else if action == "load" {
            let (saved, engine, turn) = saves.get(step["name"].as_str().unwrap()).unwrap();
            h.load(Some(saved.clone()), None, Some(engine.clone()))
                .unwrap();
            h.exec(&format!("TURN={turn}"));
            h.exec("LOCAL_PLAYER=0; HUMAN_ONE=false; RELIGION_TURN_START(0)");
            h.exec("HERO_UI_TICK()");
            loaded += 1;
        } else if action == "autoplay" || action == "end_turn" {
            h.exec(&format!(
                "RELIGION_TURN_END(0); TURN=TURN+{}; TURN_HOOK(); RELIGION_TURN_START(0)",
                step["turns"].as_i64().unwrap()
            ));
        }
    }
    assert_eq!(loaded, 10);
    assert_eq!(compared, 13);
    assert_eq!(native_founded, 1);
    assert_eq!(h.s()["revivalCount"], 1);
    assert_eq!(h.s()["state"], "PUBLIC");
}

#[test]
fn test_manual_new_game_flag_reset_handles_zero_lua_return_values() {
    let scenario = scenario();
    let step = scenario["steps"].as_array().unwrap().last().unwrap();
    assert_eq!(
        (step["action"].as_str(), step["state_name"].as_str()),
        (Some("ui_lua"), Some("FrontEnd"))
    );
    for clear in [true, false] {
        let lua = new_lua();
        lua.globals().set("CLEAR_DEFAULTS", clear).unwrap();
        lua.load("local values={}; GameConfiguration={SetValue=function(key,value) values[key]=value end, GetValue=function(key) if values[key]~=nil then return values[key] end end, SetToDefaults=function() if CLEAR_DEFAULTS then values={} end end}").exec().unwrap();
        let result = lua
            .load(step["code"].as_str().unwrap())
            .strict_eval::<Data>();
        if clear {
            assert_eq!(
                result.unwrap(),
                "manual new-game setup clears test deadlines"
            );
        } else {
            assert!(result
                .unwrap_err()
                .to_string()
                .contains("test deadlines leaked into new game defaults"));
        }
    }
}

#[test]
fn test_all_scenario_lua_chunks_compile_including_ui_and_waits() {
    let h = Harness::new();
    let scenario = scenario();
    let mut chunks = Vec::new();
    for step in scenario["steps"].as_array().unwrap() {
        if step["action"] == "ui_lua" {
            chunks.push(step["code"].as_str().unwrap());
        } else if step["action"] == "wait_for" && step["condition"]["type"] == "lua" {
            chunks.push(step["condition"]["code"].as_str().unwrap());
        }
    }
    assert_eq!(chunks.len(), 63);
    for (index, code) in chunks.iter().enumerate() {
        let result: bool = h
            .lua
            .load("local f,e=load(...); if not f then error(e) end; return true")
            .strict_call(*code)
            .unwrap_or_else(|error| panic!("scenario chunk {index}: {error}"));
        assert!(result);
    }
}
