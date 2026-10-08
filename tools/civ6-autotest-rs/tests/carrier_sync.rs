mod support_driver;
use civ6_autotest_rs::{
    carrier_sync::{compare, query, query_with_scope, Checkpoint},
    repo_root, scenario, Error,
};
use mlua::Lua;
use serde_json::json;
use std::fs;
use support_driver::{FakeBackend, Harness};

const TOKEN: &str = "txn:carrier:900";
const CANDIDATE: &str = "e977a244-test-fixture";

// Use the shipped Lua gameplay inventory, not a Rust reimplementation.
fn world(local_player: i64) -> Lua {
    world_in(Lua::new(), local_player)
}

fn world_in(lua: Lua, local_player: i64) -> Lua {
    lua.load(include_str!(
        "../../civ6-autotest-native/tests/fixtures/absorption_world.lua"
    ))
    .call::<()>(lua.create_table().unwrap())
    .unwrap();
    lua.load("function include(_) end").exec().unwrap();
    for file in ["JAC_Absorption_Rules.lua", "JAC_Absorption_Gameplay.lua"] {
        let source = fs::read_to_string(
            repo_root()
                .join("poc/JAC_PoC_Absorption/Scripts")
                .join(file),
        )
        .unwrap();
        lua.load(&source).eval::<mlua::Value>().unwrap();
    }
    lua.globals().set("LOCAL_PLAYER", local_player).unwrap();
    lua.load(r#"
        Game.GetLocalPlayer=function() return LOCAL_PLAYER end
        WORK_X=4
        Map.GetCityPlots=function() return {GetPurchasedPlots=function(_,city) return {city:GetX(),city:GetX()+1} end} end
        Cities={GetPlotPurchaseCity=function(plot) return CITIES[plot.city] end}
        GameConfiguration.IsAnyMultiplayer=function() return true end
        Game.GetGreatWorkDataFromIndex=function(index)
          assert(index==900);return {CreatorName='作者"\\\n日本',TurnCreated=3}
        end
        Game.GetGreatWorkPlayer=function(index) assert(index==900);return 1 end
        GameInfo.GreatWorks={[4]={GreatWorkType='GREATWORK_TEST',GreatWorkObjectType='GREATWORKOBJECT_RELIC',Tourism=4}}
        GameInfo.GreatWork_YieldChanges=function()
          local done=false;return function()
            if not done then done=true;return {GreatWorkType='GREATWORK_TEST',YieldType='YIELD_FAITH',YieldChange=2} end
          end
        end
        for _,city in pairs(CITIES) do
          local original=city.GetBuildings
          city.GetBuildings=function(self)
            local buildings=original(self)
            buildings.GetGreatWorkInSlot=function(_,building,slot)
              return city:GetX()==WORK_X and building==1 and slot==0 and 900 or -1
            end
            buildings.GetGreatWorkTypeFromIndex=function(_,index) assert(index==900);return 4 end
            buildings.GetCreatorNameFromIndex=function(_,index) return Game.GetGreatWorkDataFromIndex(index).CreatorName end
            buildings.GetTurnFromIndex=function(_,index) return Game.GetGreatWorkDataFromIndex(index).TurnCreated end
            return buildings
          end
        end
        local api=JAC_AbsorptionGameplay
        function TEST_STATE_EDIT(edit)
          local saved=api.GetState();edit(saved);Game:SetProperty(api.Rules.STATE_KEY,saved)
        end
        local source=assert(api.NativeWorkLocations(900)[1])
        local state=api.GetState()
        state.Transactions.checkpoint={NativeWorks={{Source=source,Carrier={Token='txn:carrier:900',
          Phase='BORROW',Route='TO_FREE',Index=900,Owner=3,Borrower=1,
          X=16,Y=0,Building=1,Slot=0,OriginalCity=CITIES[16]:GetID(),
          Source=source,Before={Name=CITIES[16]:GetName(),Population=CITIES[16]:GetPopulation(),
            Assets=api.CityAssets(CITIES[16]),Religion=api.CityReligion(CITIES[16]),Loyalty=api.CityLoyalty(CITIES[16]),
            UnitsOnCenter={},PurchasedPlots={16,17}}}}}}
        Game:SetProperty(api.Rules.STATE_KEY,state)
    "#).exec().unwrap();
    lua
}

fn checkpoint(lua: &Lua) -> Checkpoint {
    let text = lua
        .load(query_with_scope(TOKEN, CANDIDATE, false).unwrap())
        .eval::<String>()
        .unwrap();
    Checkpoint::parse(&text, TOKEN, CANDIDATE).unwrap()
}

#[test]
fn shipped_inventory_is_read_without_mutation_and_unicode_round_trips() {
    let lua = world(1);
    let before = lua.load("return TRANSFERS").eval::<i64>().unwrap();
    let record = checkpoint(&lua);
    assert_eq!(record.locations[0]["CreatorName"], "作者\"\\\n日本");
    assert_eq!(
        record.locations[0]["Effects"],
        json!({"1":"TOURISM 4","2":"YIELD_FAITH 2"})
    );
    assert_eq!(
        record.carrier["Before"]["PurchasedPlots"],
        json!({"1":16,"2":17})
    );
    assert_eq!(lua.load("return TRANSFERS").eval::<i64>().unwrap(), before);
    assert_eq!(checkpoint(&lua), record);
}

#[test]
fn two_lua_contexts_compare_peer_state_and_detect_local_divergence() {
    let left = checkpoint(&world(1));
    let peer = world(2);
    let right = checkpoint(&peer);
    compare(&left, &right, true).unwrap();
    assert!(compare(&left, &left, true).is_err());
    assert!(compare(&left, &right, false).is_err());
    peer.load("TEST_STATE_EDIT(function(s) s.Transactions.checkpoint.NativeWorks[1].Carrier.BorrowIssued=true end)").exec().unwrap();
    assert!(compare(&left, &checkpoint(&peer), true).is_err());
}

#[test]
fn reload_discards_vm_and_compares_saved_carrier_with_fresh_world_read() {
    let first = world(1);
    let before = checkpoint(&first);
    // JSON round trip stands for the recorded evidence, not a native Civ6 save.
    let saved = serde_json::to_string(&before).unwrap();
    drop(first);
    let resumed = world(1);
    compare(
        &serde_json::from_str(&saved).unwrap(),
        &checkpoint(&resumed),
        false,
    )
    .unwrap();
    for key in ["turn", "multiplayer", "city_owner", "city_id", "candidate"] {
        let mut bad = serde_json::to_value(&before).unwrap();
        bad[key] = match key {
            "multiplayer" => json!(false),
            "candidate" => json!("other-candidate"),
            _ => json!(999),
        };
        let bad: Checkpoint = serde_json::from_value(bad).unwrap();
        assert!(compare(&before, &bad, false).is_err(), "{key}");
    }
}

#[test]
fn missing_duplicate_replaced_or_incomplete_original_is_rejected() {
    let baseline = checkpoint(&world(1));
    let mut mutations = vec![];
    let mut missing = baseline.clone();
    missing.locations.clear();
    mutations.push(missing);
    let mut duplicate = baseline.clone();
    duplicate.locations.push(duplicate.locations[0].clone());
    mutations.push(duplicate);
    for key in [
        "Index",
        "GreatWorkType",
        "ObjectType",
        "CreatorName",
        "TurnCreated",
        "CreatedByPlayer",
        "Effects",
    ] {
        let mut changed = baseline.clone();
        changed.locations[0][key] = json!("changed");
        mutations.push(changed);
        let mut unknown = baseline.clone();
        unknown.carrier["Source"]
            .as_object_mut()
            .unwrap()
            .remove(key);
        mutations.push(unknown);
    }
    for bad in mutations {
        assert!(bad.validate(TOKEN, CANDIDATE).is_err());
    }
}

#[test]
fn city_changes_and_unreserved_original_location_cannot_match() {
    let baseline = checkpoint(&world(1));
    for key in [
        "Population",
        "Name",
        "Assets",
        "Religion",
        "Districts",
        "Loyalty",
        "PurchasedPlots",
        "UnitsOnCenter",
    ] {
        let mut changed = baseline.clone();
        changed.city[key] = json!("changed");
        assert!(compare(&baseline, &changed, false).is_err(), "{key}");
        let mut missing = baseline.clone();
        missing.city.as_object_mut().unwrap().remove(key);
        assert!(missing.validate(TOKEN, CANDIDATE).is_err(), "{key}");
    }
    let mut moved = baseline.clone();
    moved.locations[0]["CityID"] = json!(999);
    assert!(moved.validate(TOKEN, CANDIDATE).is_err());
}

#[test]
fn proved_requires_returned_city_and_exact_reserved_physical_slot() {
    let lua = world(1);
    lua.load(
        r#"
      WORK_X=16
      TEST_STATE_EDIT(function(s)
        local c=s.Transactions.checkpoint.NativeWorks[1].Carrier
        c.Phase='PROVED';c.FinalSlot={Owner=3,City=CITIES[16]:GetID(),X=16,Y=0,Building=1,Slot=0}
      end)
    "#,
    )
    .exec()
    .unwrap();
    let mut record = checkpoint(&lua);
    record.validate(TOKEN, CANDIDATE).unwrap();
    for key in ["Owner", "City", "X", "Y", "Building", "Slot"] {
        let mut bad = record.clone();
        bad.carrier["FinalSlot"][key] = json!(999);
        bad.ledger["Transactions"]["checkpoint"]["NativeWorks"]["1"]["Carrier"] =
            bad.carrier.clone();
        assert!(bad.validate(TOKEN, CANDIDATE).is_err(), "{key}");
    }
    record.city_owner += 1;
    assert!(record.validate(TOKEN, CANDIDATE).is_err());
}

#[test]
fn final_city_must_match_its_preborrow_baseline_even_when_peers_agree() {
    let lua = world(1);
    lua.load(
        r#"
      WORK_X=16
      TEST_STATE_EDIT(function(s)
        local c=s.Transactions.checkpoint.NativeWorks[1].Carrier
        c.Phase='PROVED';c.FinalSlot={Owner=3,City=CITIES[16]:GetID(),X=16,Y=0,Building=1,Slot=0}
      end)
    "#,
    )
    .exec()
    .unwrap();
    let good = checkpoint(&lua);
    let mut corrupt = good.clone();
    corrupt.city["Population"] = json!(99);
    assert!(compare(&corrupt, &corrupt, false).is_err());
    let mut unrelated = good.clone();
    unrelated.city["Assets"]["BUILDING_MONUMENT"]["Pillaged"] = json!(true);
    assert!(unrelated.validate(TOKEN, CANDIDATE).is_err());
}

#[test]
fn detached_or_duplicated_carrier_evidence_cannot_pass_validation() {
    let baseline = checkpoint(&world(1));
    let mut empty = baseline.clone();
    empty.ledger["Transactions"] = json!({});
    assert!(empty.validate(TOKEN, CANDIDATE).is_err());
    let mut stale = baseline.clone();
    stale.ledger["Transactions"]["checkpoint"]["NativeWorks"]["1"]["Carrier"]["BorrowIssued"] =
        json!(true);
    assert!(stale.validate(TOKEN, CANDIDATE).is_err());
    let mut double = baseline.clone();
    double.ledger["Recoveries"]["double"] = json!({"NativeCarrier":double.carrier});
    assert!(double.validate(TOKEN, CANDIDATE).is_err());
    let mut wrong = baseline.clone();
    wrong.carrier["Source"]["CreatorName"] = json!("偽の作者");
    wrong.locations[0]["CreatorName"] = json!("偽の作者");
    wrong.ledger["Transactions"]["checkpoint"]["NativeWorks"]["1"]["Carrier"] =
        wrong.carrier.clone();
    assert!(wrong.validate(TOKEN, CANDIDATE).is_err());
    let mut missing = baseline;
    missing.ledger["Transactions"]["checkpoint"]["NativeWorks"]["1"]
        .as_object_mut()
        .unwrap()
        .remove("Source");
    assert!(missing.validate(TOKEN, CANDIDATE).is_err());
    let mut no_before = checkpoint(&world(1));
    no_before.carrier.as_object_mut().unwrap().remove("Before");
    no_before.ledger["Transactions"]["checkpoint"]["NativeWorks"]["1"]["Carrier"] =
        no_before.carrier.clone();
    assert!(compare(&no_before, &no_before, false).is_err());
}

#[test]
fn flow_cases_require_the_selected_flow_and_direct_apply_is_explicit() {
    let direct = checkpoint(&world(1));
    assert!(!direct.flow_required);
    assert!(direct.flow.is_null());
    let mut required = direct.clone();
    required.flow_required = true;
    assert!(required.validate(TOKEN, CANDIDATE).is_err());
    required.flow = json!({"Flows":{},"World":{}});
    assert!(required.validate(TOKEN, CANDIDATE).is_err());
    required.flow["Flows"]["checkpoint"] = json!({"Phase":"APPLYING","Rolls":{"900":{"P1":12}}});
    required.validate(TOKEN, CANDIDATE).unwrap();
    let mut wrong = required.clone();
    wrong.flow["Flows"]["checkpoint"]["Rolls"]["900"]["P1"] = json!(13);
    assert!(compare(&required, &wrong, false).is_err());
    let mut implicit = direct;
    implicit.ledger["Transactions"]["checkpoint"]["Request"] = json!({"FlowPolicy":true});
    assert!(implicit.validate(TOKEN, CANDIDATE).is_err());
}

#[test]
fn physical_city_ids_change_during_borrow_return_and_rollback() {
    let lua = world(1);
    lua.load(
        r#"
      WORK_X=16
      TEST_STATE_EDIT(function(s)
        local c=s.Transactions.checkpoint.NativeWorks[1].Carrier
        c.Route='FROM_FREE';c.Source=JAC_AbsorptionGameplay.NativeWorkLocations(900)[1]
        s.Transactions.checkpoint.NativeWorks[1].Source=c.Source
      end)
    "#,
    )
    .exec()
    .unwrap();
    let original = checkpoint(&lua);
    lua.load("CityManager.TransferCity(CITIES[16],1)")
        .exec()
        .unwrap();
    let borrowed = checkpoint(&lua);
    assert_ne!(original.city_id, borrowed.city_id);
    assert_eq!(borrowed.city_owner, 1);
    lua.load(
        r#"
      TEST_STATE_EDIT(function(s)
        local c=s.Transactions.checkpoint.NativeWorks[1].Carrier
        c.Phase='RESTORE';c.BorrowedCity=CITIES[16]:GetID();c.MoveSource={Owner=1,City=CITIES[16]:GetID(),X=16,Y=0,Building=1,Slot=0}
      end)
    "#,
    )
    .exec()
    .unwrap();
    checkpoint(&lua);
    lua.load("CityManager.TransferCity(CITIES[16],3)")
        .exec()
        .unwrap();
    let restored = checkpoint(&lua);
    assert_ne!(restored.city_id, borrowed.city_id);
    assert_eq!(restored.city_owner, 3);

    let forward = world(1);
    forward
        .load(
            r#"
      CityManager.TransferCity(CITIES[16],1);WORK_X=16
      TEST_STATE_EDIT(function(s)
        local c=s.Transactions.checkpoint.NativeWorks[1].Carrier
        c.Phase='RETURN';c.BorrowedCity=CITIES[16]:GetID();c.MoveTarget={Owner=1,City=CITIES[16]:GetID(),X=16,Y=0,Building=1,Slot=0}
      end)
    "#,
        )
        .exec()
        .unwrap();
    checkpoint(&forward);
    forward
        .load("CityManager.TransferCity(CITIES[16],3)")
        .exec()
        .unwrap();
    let returned = checkpoint(&forward);
    assert_eq!(returned.city_owner, 3);
}

#[test]
fn from_free_proof_checks_returned_carrier_and_original_target_separately() {
    let lua = world(1);
    lua.load(
        r#"
      WORK_X=16
      TEST_STATE_EDIT(function(s)
        local snap=s.Transactions.checkpoint.NativeWorks[1]
        snap.Source=JAC_AbsorptionGameplay.NativeWorkLocations(900)[1]
        snap.Carrier.Source=snap.Source;snap.Carrier.Route='FROM_FREE'
      end)
      CityManager.TransferCity(CITIES[16],1);WORK_X=4
      CityManager.TransferCity(CITIES[16],3)
      TEST_STATE_EDIT(function(s)
        local c=s.Transactions.checkpoint.NativeWorks[1].Carrier
        c.Phase='PROVED';c.FinalSlot={Owner=3,City=CITIES[16]:GetID(),X=16,Y=0,Building=1,Slot=0}
        c.Target={Owner=1,City=CITIES[4]:GetID(),X=4,Y=0,Building=1,Slot=0}
      end)
    "#,
    )
    .exec()
    .unwrap();
    let good = checkpoint(&lua);
    for (field, keys) in [
        ("FinalSlot", ["Owner", "City", "X", "Y", "Building", "Slot"]),
        ("Target", ["Owner", "City", "X", "Y", "Building", "Slot"]),
    ] {
        for key in keys {
            let mut bad = good.clone();
            bad.carrier[field][key] = json!(999);
            bad.ledger["Transactions"]["checkpoint"]["NativeWorks"]["1"]["Carrier"] =
                bad.carrier.clone();
            assert!(bad.validate(TOKEN, CANDIDATE).is_err(), "{field}.{key}");
        }
    }
}

#[test]
fn product_recovery_prefix_requires_world_evidence_even_for_direct_apply() {
    let mut record = checkpoint(&world(1));
    let snap = record.ledger["Transactions"]["checkpoint"]["NativeWorks"]["1"].clone();
    record.ledger["Transactions"] = json!({});
    record.ledger["Recoveries"] = json!({"recovery:reward900":{"NativeCarrier":snap["Carrier"],"NativeSource":snap["Source"]}});
    assert!(!record.flow_required);
    assert!(record.validate(TOKEN, CANDIDATE).is_err());
    record.flow = json!({"Flows":{},"World":{"reward900":{"Status":"CLAIMED"}}});
    record.validate(TOKEN, CANDIDATE).unwrap();
    record.flow["World"] = json!({"other":{"Status":"CLAIMED"}});
    assert!(record.validate(TOKEN, CANDIDATE).is_err());
}

#[test]
fn duplicate_district_views_must_be_complete_and_consistent() {
    let baseline = checkpoint(&world(1));
    let mut bad = baseline.clone();
    bad.city["Districts"] = json!({});
    assert!(bad.validate(TOKEN, CANDIDATE).is_err());
    let mut missing = baseline;
    missing.city["Assets"]
        .as_object_mut()
        .unwrap()
        .remove("Districts");
    assert!(missing.validate(TOKEN, CANDIDATE).is_err());
}

#[test]
fn missing_and_duplicate_saved_intents_fail_before_recording() {
    let lua = world(1);
    let code = query("unknown", CANDIDATE).unwrap();
    assert!(lua.load(code).eval::<String>().is_err());
    lua.load("TEST_STATE_EDIT(function(s) s.Recoveries.double={NativeCarrier=s.Transactions.checkpoint.NativeWorks[1].Carrier} end)").exec().unwrap();
    assert!(lua
        .load(query(TOKEN, CANDIDATE).unwrap())
        .eval::<String>()
        .is_err());
}

#[test]
fn action_writes_evidence_and_checks_reload_without_sending_a_move() {
    let first = serde_json::to_string(&checkpoint(&world(1))).unwrap();
    let fake = FakeBackend::new().lua(move |state, code| {
        assert_eq!(state, "InGame");
        assert!(!code.contains("RequestPlayerOperation") && !code.contains("SetProperty"));
        Ok(first.clone())
    });
    let mut h = Harness::new(json!({}), fake);
    let before = json!({"action":"carrier_checkpoint","token":TOKEN,"candidate":CANDIDATE,"direct_apply":true,
        "path":"carrier/before.json","capture_as":"BORROW_BEFORE"});
    h.runner.run_step(&before).unwrap();
    let mut after = before.clone();
    after["path"] = json!("carrier/after.json");
    after.as_object_mut().unwrap().remove("capture_as");
    after["equals_capture"] = json!("BORROW_BEFORE");
    h.runner.run_step(&after).unwrap();
    let file: Checkpoint =
        serde_json::from_slice(&fs::read(h.runner.output.join("carrier/after.json")).unwrap())
            .unwrap();
    assert_eq!(file, h.runner.carrier_captures["BORROW_BEFORE"]);
    assert_eq!(h.calls().len(), 2);
}

#[test]
fn action_retains_failed_comparison_and_never_overwrites_previous_evidence() {
    let baseline = checkpoint(&world(1));
    let mut changed = baseline.clone();
    changed.ledger["PublicationSerial"] = json!(7);
    let raw = serde_json::to_string(&changed).unwrap();
    let fake = FakeBackend::new().lua(move |_, _| Ok(raw.clone()));
    let mut h = Harness::new(json!({}), fake);
    h.runner
        .carrier_captures
        .insert("BASELINE".into(), baseline);
    let step = json!({"action":"carrier_checkpoint","token":TOKEN,"candidate":CANDIDATE,"direct_apply":true,
        "path":"mismatch.json","equals_capture":"BASELINE"});
    assert!(h.runner.run_step(&step).is_err());
    let path = h.runner.output.join("mismatch.json");
    let evidence = fs::read(&path).unwrap();
    let recorded: Checkpoint = serde_json::from_slice(&evidence).unwrap();
    assert_eq!(recorded, changed);
    assert!(h.runner.run_step(&step).is_err());
    assert_eq!(fs::read(path).unwrap(), evidence);
}

#[test]
fn unknown_reply_never_retries_query_or_records_success() {
    let fake = FakeBackend::new().lua(|_, _| Err(Error::unknown("unknown reply")));
    let mut h = Harness::new(json!({}), fake);
    let step = json!({"action":"carrier_checkpoint","token":TOKEN,"candidate":CANDIDATE,"path":"before.json"});
    assert!(h.runner.run_step(&step).is_err());
    assert_eq!(h.calls().len(), 1);
    assert!(!h.runner.output.join("before.json").exists());
}

#[test]
fn schema_rejects_unsafe_paths_tokens_and_missing_input() {
    let good = json!({"action":"carrier_checkpoint","token":TOKEN,"candidate":CANDIDATE,"path":"before.json"});
    let wrap = |step| json!({"name":"carrier checkpoint","steps":[step]});
    scenario::validate_scenario(&wrap(good.clone())).unwrap();
    for key in ["token", "candidate", "path"] {
        let mut bad = good.clone();
        bad.as_object_mut().unwrap().remove(key);
        assert!(scenario::validate_scenario(&wrap(bad)).is_err(), "{key}");
    }
    for (key, value) in [
        ("token", json!("")),
        ("token", json!("x\n")),
        ("candidate", json!(false)),
        ("path", json!("../escape.json")),
        ("equals_capture", json!("unknown lower")),
    ] {
        let mut bad = good.clone();
        bad[key] = value;
        assert!(scenario::validate_scenario(&wrap(bad)).is_err());
    }
    let lua = Lua::new();
    lua.load(query("quoted'\"\\token", CANDIDATE).unwrap())
        .into_function()
        .unwrap();
}

#[test]
fn cli_compares_real_records_and_rejects_self_as_network_peer() {
    use std::process::Command;
    let root = tempfile::tempdir().unwrap();
    let left = root.path().join("left.json");
    let right = root.path().join("right.json");
    fs::write(&left, serde_json::to_vec(&checkpoint(&world(1))).unwrap()).unwrap();
    fs::write(&right, serde_json::to_vec(&checkpoint(&world(2))).unwrap()).unwrap();
    let exe = env!("CARGO_BIN_EXE_civ6-carrier-sync");
    let matched = Command::new(exe)
        .arg("compare")
        .arg(&left)
        .arg(&right)
        .arg("--peers")
        .output()
        .unwrap();
    assert!(
        matched.status.success(),
        "{}",
        String::from_utf8_lossy(&matched.stderr)
    );
    assert!(String::from_utf8_lossy(&matched.stdout).contains("MATCH"));
    let rejected = Command::new(exe)
        .arg("compare")
        .arg(&left)
        .arg(&left)
        .arg("--peers")
        .output()
        .unwrap();
    assert_eq!(rejected.status.code(), Some(1));
    let generated = Command::new(exe)
        .args(["query", TOKEN, CANDIDATE])
        .output()
        .unwrap();
    assert!(generated.status.success());
    Lua::new().load(&generated.stdout).into_function().unwrap();
}

mod native_transport {
    use super::*;
    use civ6_autotest_rs::carrier_transport as transport;
    use mlua::Function;

    fn lua_value(lua: &Lua, value: &serde_json::Value) -> mlua::Value {
        match value {
            serde_json::Value::Null => mlua::Value::Nil,
            serde_json::Value::Bool(v) => mlua::Value::Boolean(*v),
            serde_json::Value::Number(v) => mlua::Value::Integer(v.as_i64().unwrap()),
            serde_json::Value::String(v) => mlua::Value::String(lua.create_string(v).unwrap()),
            serde_json::Value::Object(map) => {
                let table = lua.create_table().unwrap();
                for (k, v) in map {
                    table.set(k.as_str(), lua_value(lua, v)).unwrap();
                }
                mlua::Value::Table(table)
            }
            serde_json::Value::Array(_) => panic!("boundary packet has no arrays"),
        }
    }
    fn ready() -> Lua {
        let lua = world(2);
        lua.load(r#"
          TEST_STATE_EDIT(function(s) s.Transactions.checkpoint.NativeWorks[1].Carrier.BorrowToken='txn:carrier:900:borrow' end)
          for id=1,3 do Players[id].IsTurnActive=function() return true end end
          GameEvents.JAC_CarrierBoundary_Operation={Add=function(fn) TEST_RECEIVE=fn end}
          GameEvents.JAC_CarrierMP_Operation={Add=function(fn) TEST_MP_RECEIVE=fn end}
        "#).exec().unwrap();
        lua.load(
            fs::read_to_string(
                repo_root().join("poc/JAC_PoC_Absorption/Scripts/JAC_CarrierBoundary.lua"),
            )
            .unwrap(),
        )
        .exec()
        .unwrap();
        lua
    }
    fn receive(lua: &Lua, actor: i64, packet: &serde_json::Value) -> bool {
        lua.globals()
            .get::<Function>("TEST_RECEIVE")
            .unwrap()
            .call((actor, lua_value(lua, packet)))
            .unwrap()
    }
    fn message(lua: &Lua) -> serde_json::Value {
        transport::packet(&checkpoint(lua), &[2]).unwrap()
    }

    fn mp_world(local: i64, human_donor: bool) -> Lua {
        let lua = ready();
        lua.globals().set("LOCAL_PLAYER", local).unwrap();
        lua.globals().set("HUMAN_DONOR", human_donor).unwrap();
        lua.load(r#"
          Players[1].IsHuman=function() return HUMAN_DONOR end
          Players[1].GetCities=function()
            return {GetCount=function() return 2 end,GetCapitalCity=function() return CITIES[8] end,
              Members=function() return pairs(Players[1].cities) end,
              FindID=function(_,id) for _,c in pairs(Players[1].cities) do if c.id==id then return c end end end}
          end
          for _,u in pairs(Players[1].units) do u.x=8 end
          CityManager.TransferCity(CITIES[16],2);TRANSFERS=0
          JAC_AbsorptionGameplay.RegisterCityProperty('MP_TEST_PROPERTY')
          CITIES[4]:SetProperty('MP_TEST_PROPERTY',{Value='復元',Culture=123456789012345.67,Accumulation=1.0000000000000002})
          for _,c in pairs(CITIES) do
            local old=c.GetBuildings
            c.GetBuildings=function(self)
              local b=old(self);b.GetGreatWorkSlotType=function() return 1 end;return b
            end
          end
          GameInfo.GreatWorkSlotTypes={[1]={GreatWorkSlotType='GREATWORKSLOT_RELIC'}}
          GameInfo.GreatWork_ValidSubTypes=function()
            local done=false;return function() if not done then done=true;return {
              GreatWorkObjectType='GREATWORKOBJECT_RELIC',GreatWorkSlotType='GREATWORKSLOT_RELIC'} end end
          end
          TEST_STATE_EDIT(function(s)
            local tx=s.Transactions.checkpoint;tx.ID='checkpoint';tx.To=2;tx.From=1
            tx.Turn=CURRENT_TURN;tx.Request={};tx.Adapters={};tx.Status='PREPARED'
            local snap=tx.NativeWorks[1];snap.Carrier=nil;snap.Source.BuildingType='BUILDING_TEST'
            snap.Custodian=2;snap.Destination=2;snap.NativeRoute='CARRIER'
          end)
          ExposedMembers.JAC_HeroReadAPI={ReadCarrierPlots=function(owner,id)
            local c=assert(Players[owner]:GetCities():FindID(id));return {c:GetX(),c:GetX()+1}
          end}
          ExposedMembers.JAC_HeroReadAPI.FindCompatibleNativeSlots=function()
            return {{Owner=2,City=CITIES[16]:GetID(),X=16,Y=0,Building=1,Slot=0,SlotType='GREATWORKSLOT_RELIC'}}
          end
          PlayerOperations={MOVE_GREAT_WORK='move',EXECUTE_SCRIPT='script',PARAM_PLAYER_ONE='owner',
            PARAM_CITY_SRC='source',PARAM_CITY_DEST='target',PARAM_BUILDING_SRC='sb',
            PARAM_BUILDING_DEST='tb',PARAM_GREAT_WORK_INDEX='index',PARAM_SLOT='slot'}
          PACKETS={};UI_MOVES=0
          UI_SESSION_CLOCK=0
          UI={GetElapsedTime=function() UI_SESSION_CLOCK=UI_SESSION_CLOCK+1;return UI_SESSION_CLOCK end,
            RequestPlayerOperation=function(actor,op,p)
            assert(actor==2)
            if op=='script' then PACKETS[#PACKETS+1]=p;return true end
            assert(op=='move' and p.owner==actor and CITIES[4]:GetOwner()==actor
              and CITIES[16]:GetOwner()==actor and p.source==CITIES[4]:GetID()
              and p.target==CITIES[16]:GetID() and p.index==900 and p.slot==0)
            UI_MOVES=UI_MOVES+1;WORK_X=16;return true
          end}
          function MP_PLAN()
            local s=JAC_AbsorptionGameplay.GetState();local tx=s.Transactions.checkpoint
            return JAC_CarrierMP.AdvanceSnap(s,tx,tx.NativeWorks[1])
          end
          function MP_OBSERVE()
            return JAC_ObserveNativeCarrierMP(JAC_AbsorptionGameplay.GetState().Transactions.checkpoint.NativeWorks[1].Carrier)
          end
          function MP_LAST() return JAC_CarrierMP.Encode(PACKETS[#PACKETS]) end
          function MP_RECEIVE(raw)
            local get=Game.GetLocalPlayer;local reads=ExposedMembers.JAC_HeroReadAPI
            Game.GetLocalPlayer=function() error('Gameplay local identity reached') end
            ExposedMembers.JAC_HeroReadAPI=nil
            local result=TEST_MP_RECEIVE(2,JAC_CarrierMP.Decode(raw))
            Game.GetLocalPlayer=get;ExposedMembers.JAC_HeroReadAPI=reads
            return result
          end
        "#).exec().unwrap();
        lua.load(
            fs::read_to_string(repo_root().join("poc/JAC_PoC_Absorption/UI/JAC_CarrierMP_UI.lua"))
                .unwrap(),
        )
        .exec()
        .unwrap();
        lua
    }

    fn mp_receive(lua: &Lua, raw: &str) -> bool {
        lua.globals()
            .get::<Function>("MP_RECEIVE")
            .unwrap()
            .call(raw)
            .unwrap()
    }
    fn mp_publish(lua: &Lua) -> String {
        assert!(lua.load("return MP_OBSERVE()").eval::<bool>().unwrap());
        lua.load("return MP_LAST()").eval().unwrap()
    }

    fn mp_until(lua: &Lua, phase: &str) {
        for _ in 0..12 {
            let current=lua.load("return JAC_AbsorptionGameplay.GetState().Transactions.checkpoint.NativeWorks[1].Carrier.MP.Phase").eval::<String>().unwrap();
            if current == phase {
                return;
            }
            let raw = mp_publish(lua);
            assert!(mp_receive(lua, &raw), "{current} -> {phase}");
        }
        panic!("did not reach {phase}");
    }

    #[test]
    fn mp_review_rejects_ui_property_injection_and_uses_core_catalog() {
        for change in [
            "o.Cities[1].Properties.EVIL=9",
            "o.Cities[1].Properties.MP_TEST_PROPERTY.Value='forged'",
            "o.Cities[1].PropertyKeys.EVIL=true;o.Cities[1].Properties.EVIL=9",
            "o.Cities[1].Properties.MP_TEST_PROPERTY=nil",
        ] {
            let lua = mp_world(2, false);
            lua.load("MP_PLAN()").exec().unwrap();
            mp_until(&lua, "PREPARE");
            let raw = mp_publish(&lua);
            lua.globals().set("REVIEW_PACKET", raw).unwrap();
            let bad=lua.load(format!("local p=JAC_CarrierMP.Decode(REVIEW_PACKET);local o=JAC_CarrierMP.Decode(p.Payload);{change};p.Payload=JAC_CarrierMP.Encode(o);return JAC_CarrierMP.Encode(p)")).eval::<String>().unwrap();
            assert!(!mp_receive(&lua, &bad), "{change}");
            lua.load("assert(TRANSFERS==0 and CITIES[4]:GetProperty('EVIL')==nil and CITIES[4]:GetProperty('MP_TEST_PROPERTY').Value=='復元')").exec().unwrap();
        }
    }

    #[test]
    fn mp_review_partial_second_loan_failure_returns_only_the_first_loan() {
        let lua = mp_world(2, false);
        lua.load(r#"
          CityManager.TransferCity(CITIES[16],3);CityManager.TransferCity(CITIES[0],3);TRANSFERS=0
          local cities=Players[3].GetCities
          Players[3].GetCities=function() local value=cities();value.GetCapitalCity=function() return CITIES[0] end;return value end
          TEST_STATE_EDIT(function(s) local snap=s.Transactions.checkpoint.NativeWorks[1];snap.Custodian=3;snap.Destination=3 end)
          ExposedMembers.JAC_HeroReadAPI.FindCompatibleNativeSlots=function()
            return {{Owner=3,City=CITIES[16]:GetID(),X=16,Y=0,Building=1,Slot=0,SlotType='GREATWORKSLOT_RELIC'}} end
          local transfer=CityManager.TransferCity
          CityManager.TransferCity=function(c,owner)
            if c:GetX()==16 and owner==2 then error('second loan rejected') end
            return transfer(c,owner)
          end
          MP_PLAN()
        "#).exec().unwrap();
        mp_until(&lua, "PREPARE");
        let raw = mp_publish(&lua);
        assert!(mp_receive(&lua, &raw));
        lua.load(
            r#"
          local c=JAC_AbsorptionGameplay.GetState().Transactions.checkpoint.NativeWorks[1].Carrier
          assert(c.MP.Phase=='ABORT_RESTORED' and TRANSFERS==2 and UI_MOVES==0)
          assert(CITIES[4]:GetOwner()==1 and CITIES[16]:GetOwner()==3 and WORK_X==4)
          assert(CITIES[4]:GetProperty('MP_TEST_PROPERTY').Value=='復元')
        "#,
        )
        .exec()
        .unwrap();
        let raw = mp_publish(&lua);
        assert!(mp_receive(&lua, &raw));
    }

    #[test]
    fn mp_review_inactive_human_and_manual_next_turn_are_waits() {
        let lua = mp_world(2, false);
        lua.load("MP_PLAN();Players[2].IsTurnActive=function() return false end")
            .exec()
            .unwrap();
        lua.load("local ok,result=pcall(MP_PLAN);assert(ok and result==false,'inactive human must wait')").exec().unwrap();
        lua.load(
            r#"
          local tx=JAC_AbsorptionGameplay.Resume('checkpoint');assert(tx.Status=='WAITING_UI')
          CURRENT_TURN=CURRENT_TURN+1
          tx=JAC_AbsorptionGameplay.Resume('checkpoint');assert(tx.Status=='WAITING_UI')
          assert(TRANSFERS==0 and UI_MOVES==0)
        "#,
        )
        .exec()
        .unwrap();
        lua.load(r#"
          TEST_STATE_EDIT(function(s) s.Transactions.manual={ID='manual',Status='PREPARED',Turn=CURRENT_TURN-1,Request={}} end)
          assert(not pcall(JAC_AbsorptionGameplay.Resume,'manual'))
        "#).exec().unwrap();
    }

    #[test]
    fn mp_review_preflight_checks_later_unsupported_items_before_any_loan() {
        for code in [
            "extra.Source.BuildingType='BUILDING_PALACE'",
            "extra.Source.X=8;extra.Source.CityID=CITIES[8].id",
        ] {
            let lua = mp_world(2, false);
            lua.load(format!("TEST_STATE_EDIT(function(s) local tx=s.Transactions.checkpoint;local extra=JAC_AbsorptionRules.Copy(tx.NativeWorks[1]);extra.Source.Index=901;{code};tx.NativeWorks[2]=extra end)")).exec().unwrap();
            assert!(!lua.load("return pcall(MP_PLAN)").eval::<bool>().unwrap());
            lua.load("assert(TRANSFERS==0 and UI_MOVES==0 and JAC_AbsorptionGameplay.GetState().Transactions.checkpoint.NativeWorks[1].Carrier==nil)").exec().unwrap();
        }
    }

    #[test]
    fn mp_review_core_selects_second_candidate_when_first_is_a_capital() {
        let lua = mp_world(2, false);
        lua.load(r#"
          CityManager.TransferCity(CITIES[16],3);CityManager.TransferCity(CITIES[0],3);TRANSFERS=0
          local cities=Players[3].GetCities
          Players[3].GetCities=function() local value=cities();value.GetCapitalCity=function() return CITIES[0] end;return value end
          TEST_STATE_EDIT(function(s) local snap=s.Transactions.checkpoint.NativeWorks[1];snap.Custodian=3;snap.Destination=3 end)
          ExposedMembers.JAC_HeroReadAPI.FindCompatibleNativeSlots=function()
            return {{Owner=3,City=CITIES[0]:GetID(),X=0,Y=0,Building=1,Slot=0,SlotType='GREATWORKSLOT_RELIC'},
              {Owner=3,City=CITIES[16]:GetID(),X=16,Y=0,Building=1,Slot=0,SlotType='GREATWORKSLOT_RELIC'}} end
          MP_PLAN()
        "#).exec().unwrap();
        let raw = mp_publish(&lua);
        assert!(mp_receive(&lua, &raw));
        lua.load("local c=JAC_AbsorptionGameplay.GetState().Transactions.checkpoint.NativeWorks[1].Carrier;assert(c.X==16 and c.MP.Phase=='PREPARE' and TRANSFERS==0)").exec().unwrap();
    }

    #[test]
    fn mp_review_session_reset_before_claim_cancels_and_fences_late_claim() {
        let lua = mp_world(2, false);
        lua.load("MP_PLAN()").exec().unwrap();
        mp_until(&lua, "READY");
        let old_claim = mp_publish(&lua); // produced by old session, not yet received
        lua.load(transport::ui_binding()).exec().unwrap();
        let abort = mp_publish(&lua);
        assert!(mp_receive(&lua, &abort));
        assert!(!mp_receive(&lua, &old_claim));
        lua.load("assert(UI_MOVES==0 and TRANSFERS==2 and CITIES[4]:GetOwner()==1 and WORK_X==4)")
            .exec()
            .unwrap();
        let final_report = mp_publish(&lua);
        assert!(mp_receive(&lua, &final_report));
    }

    #[test]
    fn mp_natural_turn_changes_can_finish_borrowed_ready_dispatch_and_restored_phases() {
        for phase in ["BORROWED", "READY", "DISPATCH", "RESTORED"] {
            let lua = mp_world(2, false);
            lua.load("MP_PLAN()").exec().unwrap();
            mp_until(
                &lua,
                if phase == "RESTORED" {
                    "DISPATCH"
                } else {
                    phase
                },
            );
            if phase == "RESTORED" {
                lua.load("MP_OBSERVE()").exec().unwrap();
                assert!(mp_receive(&lua, &mp_publish(&lua)));
            }
            lua.load(
                r#"
              CURRENT_TURN=CURRENT_TURN+1
              for _,x in ipairs({4,16}) do
                local c=CITIES[x];c.pop=c.pop+1;c.loyalty=c.loyalty-0.25
                c.faith={['5']=3,['6']=2}
                c:SetProperty('MP_TEST_PROPERTY',{Value='自然更新',Culture=17.25})
              end
            "#,
            )
            .exec()
            .unwrap();
            for _ in 0..10 {
                let current=lua.load("return JAC_AbsorptionGameplay.GetState().Transactions.checkpoint.NativeWorks[1].Carrier.MP.Phase").eval::<String>().unwrap();
                if current == "DONE" {
                    break;
                }
                let published = lua.load("return MP_OBSERVE()").eval::<bool>().unwrap();
                if published {
                    assert!(
                        mp_receive(
                            &lua,
                            &lua.load("return MP_LAST()").eval::<String>().unwrap()
                        ),
                        "{phase}: {current}"
                    );
                }
            }
            lua.load(r#"
              local m=JAC_AbsorptionGameplay.GetState().Transactions.checkpoint.NativeWorks[1].Carrier.MP
              assert(m.Phase=='DONE' and UI_MOVES==1 and TRANSFERS==2 and WORK_X==16)
              assert(CITIES[4]:GetOwner()==1 and CITIES[16]:GetOwner()==2)
              for _,x in ipairs({4,16}) do
                assert(CITIES[x].pop==7 and CITIES[x].loyalty==86.75)
                assert(CITIES[x]:GetProperty('MP_TEST_PROPERTY').Value=='自然更新')
              end
            "#).exec().unwrap();
        }
    }

    #[test]
    fn mp_one_stale_ui_observation_waits_without_aborting_and_fresh_data_recovers() {
        let lua = mp_world(2, false);
        lua.load("MP_PLAN()").exec().unwrap();
        mp_until(&lua, "BORROWED");
        let fresh = mp_publish(&lua);
        lua.globals().set("UI_FRESH", fresh.clone()).unwrap();
        let stale=lua.load("local p=JAC_CarrierMP.Decode(UI_FRESH);local o=JAC_CarrierMP.Decode(p.Payload);o.Cities[1].Owner=1;o.Cities[1].ID=4;p.Payload=JAC_CarrierMP.Encode(o);return JAC_CarrierMP.Encode(p)").eval::<String>().unwrap();
        assert!(!mp_receive(&lua, &stale));
        lua.load("local m=JAC_AbsorptionGameplay.GetState().Transactions.checkpoint.NativeWorks[1].Carrier.MP;assert(m.Phase=='BORROWED' and m.StaleCount==1 and TRANSFERS==1 and CITIES[4]:GetOwner()==2)").exec().unwrap();
        assert!(mp_receive(&lua, &fresh));
        lua.load("local m=JAC_AbsorptionGameplay.GetState().Transactions.checkpoint.NativeWorks[1].Carrier.MP;assert(m.Phase=='READY' and m.StaleCount==nil and TRANSFERS==1)").exec().unwrap();
    }

    #[test]
    fn mp_persistent_stale_ui_has_a_bounded_abort_cycle_and_never_dispatches_move() {
        let lua = mp_world(2, false);
        lua.load("MP_PLAN()").exec().unwrap();
        for cycle in 0..3 {
            mp_until(&lua, "BORROWED");
            let fresh = mp_publish(&lua);
            lua.globals().set("UI_FRESH", fresh).unwrap();
            let stale=lua.load("local p=JAC_CarrierMP.Decode(UI_FRESH);local o=JAC_CarrierMP.Decode(p.Payload);o.Cities[1].Owner=1;p.Payload=JAC_CarrierMP.Encode(o);return JAC_CarrierMP.Encode(p)").eval::<String>().unwrap();
            for _ in 0..3 {
                assert!(!mp_receive(&lua, &stale));
            }
            assert!(mp_receive(&lua, &mp_publish(&lua))); // observe restored originals
            assert!(mp_receive(&lua, &mp_publish(&lua))); // resume, or stop at cap
            if cycle == 2 {
                lua.load("local m=JAC_AbsorptionGameplay.GetState().Transactions.checkpoint.NativeWorks[1].Carrier.MP;assert(m.Phase=='ABORT_DONE' and m.WaitingFor=='MP_ABORT_LIMIT' and m.AbortCycles==3 and TRANSFERS==6 and UI_MOVES==0 and CITIES[4]:GetOwner()==1)").exec().unwrap();
                lua.load("for i=1,100 do assert(not MP_OBSERVE());MP_PLAN() end;assert(TRANSFERS==6 and UI_MOVES==0)").exec().unwrap();
            }
        }
    }

    #[test]
    fn mp_partial_return_keeps_natural_updates_on_the_already_restored_city() {
        let lua = mp_world(2, false);
        lua.load(r#"
          CityManager.TransferCity(CITIES[16],3);CityManager.TransferCity(CITIES[0],3);TRANSFERS=0
          local cities=Players[3].GetCities
          Players[3].GetCities=function() local value=cities();value.GetCapitalCity=function() return CITIES[0] end;return value end
          TEST_STATE_EDIT(function(s) local snap=s.Transactions.checkpoint.NativeWorks[1];snap.Custodian=3;snap.Destination=3 end)
          ExposedMembers.JAC_HeroReadAPI.FindCompatibleNativeSlots=function()
            return {{Owner=3,City=CITIES[16]:GetID(),X=16,Y=0,Building=1,Slot=0,SlotType='GREATWORKSLOT_RELIC'}} end
          local transfer=CityManager.TransferCity
          CityManager.TransferCity=function(c,owner)
            if FAIL_SECOND_RETURN and c:GetX()==16 and owner==3 then error('second return blocked') end
            return transfer(c,owner)
          end
          MP_PLAN()
        "#).exec().unwrap();
        mp_until(&lua, "DISPATCH");
        lua.load("MP_OBSERVE();FAIL_SECOND_RETURN=true")
            .exec()
            .unwrap();
        assert!(mp_receive(&lua, &mp_publish(&lua)));
        lua.load(
            r#"
          assert(CITIES[4]:GetOwner()==1 and CITIES[16]:GetOwner()==2)
          CURRENT_TURN=CURRENT_TURN+1
          CITIES[4]:SetProperty('MP_TEST_PROPERTY',{Value='返還済み都市の更新'})
          CITIES[16]:SetProperty('MP_TEST_PROPERTY',{Value='未返還都市の更新'})
          FAIL_SECOND_RETURN=false
        "#,
        )
        .exec()
        .unwrap();
        assert!(mp_receive(&lua, &mp_publish(&lua)));
        lua.load("assert(CITIES[16]:GetOwner()==3 and CITIES[4]:GetProperty('MP_TEST_PROPERTY').Value=='返還済み都市の更新' and CITIES[16]:GetProperty('MP_TEST_PROPERTY').Value=='未返還都市の更新' and UI_MOVES==1)").exec().unwrap();
        assert!(mp_receive(&lua, &mp_publish(&lua)));
    }

    #[test]
    fn mp_invalid_preclaim_observation_recovers_cities_and_fences_the_pending_claim() {
        let lua = mp_world(2, false);
        lua.load("MP_PLAN()").exec().unwrap();
        mp_until(&lua, "READY");
        let claim = mp_publish(&lua);
        lua.globals().set("PENDING_CLAIM", claim.clone()).unwrap();
        let bad=lua.load("local p=JAC_CarrierMP.Decode(PENDING_CLAIM);local o=JAC_CarrierMP.Decode(p.Payload);o.Cities[1].Properties.EVIL=9;p.Payload=JAC_CarrierMP.Encode(o);return JAC_CarrierMP.Encode(p)").eval::<String>().unwrap();
        assert!(!mp_receive(&lua, &bad));
        lua.load("assert(CITIES[4]:GetOwner()==1 and UI_MOVES==0 and TRANSFERS==2 and CITIES[4]:GetProperty('EVIL')==nil)").exec().unwrap();
        assert!(!mp_receive(&lua, &claim));
        assert!(mp_receive(&lua, &mp_publish(&lua)));
    }

    #[test]
    fn mp_stale_manual_select_or_prepare_cannot_begin_a_new_loan() {
        for phase in ["SELECT", "PREPARE"] {
            let lua = mp_world(2, false);
            lua.load("MP_PLAN()").exec().unwrap();
            if phase == "PREPARE" {
                mp_until(&lua, phase);
            }
            let packet = mp_publish(&lua);
            lua.globals().set("STALE_PACKET", packet).unwrap();
            lua.load("CURRENT_TURN=CURRENT_TURN+1").exec().unwrap();
            let packet=lua.load("local p=JAC_CarrierMP.Decode(STALE_PACKET);p.Turn=CURRENT_TURN;return JAC_CarrierMP.Encode(p)").eval::<String>().unwrap();
            assert!(!mp_receive(&lua, &packet));
            lua.load("assert(TRANSFERS==0 and UI_MOVES==0 and CITIES[4]:GetOwner()==1)")
                .exec()
                .unwrap();
        }
    }

    #[test]
    fn mp_review_saved_loan_crosses_turn_only_for_carrier_cleanup() {
        let lua = mp_world(2, false);
        lua.load("MP_PLAN()").exec().unwrap();
        mp_until(&lua, "READY");
        lua.load(
            r#"
          CURRENT_TURN=CURRENT_TURN+1
          Players[2].IsTurnActive=function() return false end
          local tx=JAC_AbsorptionGameplay.Resume('checkpoint')
          assert(tx.Status=='WAITING_UI' and CITIES[4]:GetOwner()==2 and TRANSFERS==1)
          Players[2].IsTurnActive=function() return true end
          tx=JAC_AbsorptionGameplay.Resume('checkpoint')
          assert(tx.Status=='WAITING_UI' and tx.WaitingFor=='MP_CARRIER_CONTINUATION_ONLY')
        "#,
        )
        .exec()
        .unwrap();
        lua.load(transport::ui_binding()).exec().unwrap();
        let cancel = mp_publish(&lua);
        assert!(mp_receive(&lua, &cancel));
        let restored = mp_publish(&lua);
        assert!(mp_receive(&lua, &restored));
        lua.load("assert(CITIES[4]:GetOwner()==1 and TRANSFERS==2 and UI_MOVES==0)")
            .exec()
            .unwrap();
    }

    #[test]
    fn mp_review_unknown_queued_move_is_not_retried_or_rolled_back_early() {
        let lua = mp_world(2, false);
        lua.load("MP_PLAN()").exec().unwrap();
        mp_until(&lua, "DISPATCH");
        lua.load(
            r#"
          local request=UI.RequestPlayerOperation
          UI.RequestPlayerOperation=function(actor,op,p)
            if op=='move' then UI_MOVES=UI_MOVES+1;QUEUED_MOVE=true;error('send response lost') end
            return request(actor,op,p)
          end
        "#,
        )
        .exec()
        .unwrap();
        assert!(!lua.load("return MP_OBSERVE()").eval::<bool>().unwrap());
        lua.load(transport::ui_binding()).exec().unwrap();
        for _ in 0..3 {
            let raw = mp_publish(&lua);
            assert!(mp_receive(&lua, &raw));
        }
        lua.load(r#"
          local m=JAC_AbsorptionGameplay.GetState().Transactions.checkpoint.NativeWorks[1].Carrier.MP
          assert(m.Phase=='DISPATCH' and m.WaitingFor=='NATIVE_QUEUE_RESOLUTION')
          assert(UI_MOVES==1 and TRANSFERS==1 and CITIES[4]:GetOwner()==2 and WORK_X==4)
          WORK_X=16 -- the already-queued engine MOVE finally executes
        "#).exec().unwrap();
        let moved = mp_publish(&lua);
        assert!(mp_receive(&lua, &moved));
        let restored = mp_publish(&lua);
        assert!(mp_receive(&lua, &restored));
        lua.load("assert(UI_MOVES==1 and TRANSFERS==2 and CITIES[4]:GetOwner()==1)")
            .exec()
            .unwrap();
    }

    #[test]
    fn mp_review_explicit_move_rejection_restores_without_resending() {
        let lua = mp_world(2, false);
        lua.load("MP_PLAN()").exec().unwrap();
        mp_until(&lua, "DISPATCH");
        lua.load(
            r#"
          local request=UI.RequestPlayerOperation
          UI.RequestPlayerOperation=function(actor,op,p)
            if op=='move' then UI_MOVES=UI_MOVES+1;return false end
            return request(actor,op,p)
          end
        "#,
        )
        .exec()
        .unwrap();
        assert!(!lua.load("return MP_OBSERVE()").eval::<bool>().unwrap());
        let raw = mp_publish(&lua);
        assert!(mp_receive(&lua, &raw));
        lua.load("assert(CITIES[4]:GetOwner()==1 and TRANSFERS==2 and UI_MOVES==1 and WORK_X==4)")
            .exec()
            .unwrap();
        let restored = mp_publish(&lua);
        assert!(mp_receive(&lua, &restored));
    }

    #[test]
    fn mp_human_and_ai_sources_use_one_owned_move_and_restore_in_two_vms() {
        for (human, paired) in [(true, false), (false, false), (false, true)] {
            let left = mp_world(2, human);
            let mut right = mp_world(3, human);
            if paired {
                for lua in [&left, &right] {
                    lua.load(r#"
                  CityManager.TransferCity(CITIES[16],3);CityManager.TransferCity(CITIES[0],3);TRANSFERS=0
                  local cities=Players[3].GetCities
                  Players[3].GetCities=function() local value=cities();value.GetCapitalCity=function() return CITIES[0] end;return value end
                  TEST_STATE_EDIT(function(s) local snap=s.Transactions.checkpoint.NativeWorks[1];snap.Custodian=3;snap.Destination=3 end)
                  ExposedMembers.JAC_HeroReadAPI.FindCompatibleNativeSlots=function()
                    return {{Owner=3,City=CITIES[16]:GetID(),X=16,Y=0,Building=1,Slot=0,SlotType='GREATWORKSLOT_RELIC'}} end
                "#).exec().unwrap();
                }
            }
            for lua in [&left, &right] {
                assert!(!lua.load("return MP_PLAN()").eval::<bool>().unwrap());
            }
            for _ in 0..4 {
                let raw = mp_publish(&left);
                for lua in [&right, &left] {
                    assert!(mp_receive(lua, &raw));
                    let before = lua.load("return TRANSFERS").eval::<i64>().unwrap();
                    let _ = mp_receive(lua, &raw);
                    assert_eq!(lua.load("return TRANSFERS").eval::<i64>().unwrap(), before);
                }
            }
            // The same human owns both city IDs before the genuine MOVE request.
            assert!(!left.load("return MP_OBSERVE()").eval::<bool>().unwrap());
            assert_eq!(left.load("return UI_MOVES").eval::<i64>().unwrap(), 1);
            assert!(!right.load("return MP_OBSERVE()").eval::<bool>().unwrap());
            right.load("WORK_X=16").exec().unwrap(); // replicated native engine effect
                                                     // Recreate a whole peer VM from saved Gameplay + engine city state,
                                                     // not merely a JSON copy compared within the original VM.
            let saved=right.load(r#"
              local cities={};for x,c in pairs(CITIES) do cities[x]={Owner=c.owner,ID=c.id,Properties=c.props} end
              return JAC_CarrierMP.Encode({State=JAC_AbsorptionGameplay.GetState(),Cities=cities,Work=WORK_X,Transfers=TRANSFERS})
            "#).eval::<String>().unwrap();
            right = mp_world(3, human);
            right.globals().set("MP_SAVED", saved).unwrap();
            right.load(r#"
              local saved=JAC_CarrierMP.Decode(MP_SAVED)
              for _,p in pairs(Players) do p.cities={} end
              for x,record in pairs(saved.Cities) do
                local c=CITIES[x];c.owner=record.Owner;c.id=record.ID;c.props=record.Properties
                Players[c.owner].cities[x]=c
              end
              for _,plot in pairs(PLOTS) do if plot.city then plot.owner=CITIES[plot.city].owner end end
              WORK_X=saved.Work;TRANSFERS=saved.Transfers
              Game:SetProperty(JAC_AbsorptionGameplay.Rules.STATE_KEY,saved.State)
            "#).exec().unwrap();
            for _ in 0..2 {
                let raw = mp_publish(&left);
                for lua in [&left, &right] {
                    assert!(mp_receive(lua, &raw));
                    assert!(mp_receive(lua, &raw));
                }
            }
            for lua in [&left, &right] {
                assert!(lua.load("return MP_PLAN()").eval::<bool>().unwrap());
                lua.load(r#"
                  local c=JAC_AbsorptionGameplay.GetState().Transactions.checkpoint.NativeWorks[1].Carrier
                  assert(CITIES[4]:GetOwner()==1 and CITIES[16]:GetOwner()==c.Owner)
                  assert(TRANSFERS==(c.Owner==2 and 2 or 4))
                  assert(CITIES[4]:GetProperty('MP_TEST_PROPERTY').Value=='復元')
                  assert(CITIES[4]:GetProperty('MP_TEST_PROPERTY').Culture==123456789012345.67)
                  assert(CITIES[4]:GetProperty('MP_TEST_PROPERTY').Accumulation==1.0000000000000002)
                  assert(#JAC_AbsorptionGameplay.NativeWorkLocations(900)==1 and WORK_X==16)
                  assert(JAC_AbsorptionGameplay.GetState().Transactions.checkpoint.NativeWorks[1].PhysicalMoved)
                  assert(not MP_OBSERVE())
                "#).exec().unwrap();
            }
            assert_eq!(left.load("return UI_MOVES").eval::<i64>().unwrap(), 1);
            let state = "return JAC_AbsorptionRules.Canonical(JAC_AbsorptionGameplay.GetState())";
            assert_eq!(
                left.load(state).eval::<String>().unwrap(),
                right.load(state).eval::<String>().unwrap()
            );
        }
    }

    fn palace_world(local: i64, human: bool) -> Lua {
        let lua = mp_world(local, human);
        lua.load(r#"
          CITIES[4].palace=true
          GameInfo.GreatWorks.GREATWORK_TEST=GameInfo.GreatWorks[4]
          local _,guard=next(Players[1].units);assert(guard);guard.x=4
          CityManager.TransferCity(CITIES[8],0);TRANSFERS=0
          print=function(message) PALACE_DIAG=tostring(message) end
          Players[1].GetCities=function()
            return {GetCount=function() return 1 end,GetCapitalCity=function() return CITIES[4] end,
              Members=function() return pairs(Players[1].cities) end,
              FindID=function(_,id) for _,c in pairs(Players[1].cities) do if c.id==id then return c end end end}
          end
          for _,c in pairs(CITIES) do
            local old=c.GetBuildings
            c.GetBuildings=function(self)
              local b=old(self)
              b.GetGreatWorkInSlot=function(_,building,slot)
                return c.x==WORK_X and building==(WORK_X==4 and 0 or 1) and slot==0 and 900 or -1
              end
              return b
            end
          end
          TEST_STATE_EDIT(function(s)
            local source=assert(JAC_AbsorptionGameplay.NativeWorkLocations(900)[1]);assert(source.BuildingType=='BUILDING_PALACE')
            s.Transactions.checkpoint.NativeWorks[1].Source=source
          end)
          local original=CityManager.TransferCity
          CityManager.TransferCity=function(c,owner)
            assert(c.x~=4,'Palace capital must never be borrowed');return original(c,owner)
          end
          UI.RequestPlayerOperation=function(actor,op,p)
            assert(actor==1,'Palace MOVE belongs to its human owner')
            if op=='script' then PACKETS[#PACKETS+1]=p;return true end
            assert(op=='move' and p.owner==1 and CITIES[4].owner==1 and CITIES[16].owner==1)
            assert(p.source==CITIES[4].id and p.target==CITIES[16].id and p.sb==0 and p.tb==1 and p.index==900)
            UI_MOVES=UI_MOVES+1;WORK_X=16;return true
          end
          function MP_RECEIVE(raw)
            local get=Game.GetLocalPlayer;local reads=ExposedMembers.JAC_HeroReadAPI
            Game.GetLocalPlayer=function() error('Gameplay local identity reached') end
            ExposedMembers.JAC_HeroReadAPI=nil
            local result=TEST_MP_RECEIVE(1,JAC_CarrierMP.Decode(raw))
            Game.GetLocalPlayer=get;ExposedMembers.JAC_HeroReadAPI=reads;return result
          end
        "#).exec().unwrap();
        lua
    }

    #[test]
    fn mp_human_palace_owner_moves_original_without_borrowing_last_capital() {
        let left = palace_world(1, true);
        let mut right = palace_world(2, true);
        for lua in [&left, &right] {
            assert!(!lua.load("return MP_PLAN()").eval::<bool>().unwrap());
        }
        for step in 0..12 {
            let phase = left.load("return JAC_AbsorptionGameplay.GetState().Transactions.checkpoint.NativeWorks[1].Carrier.MP.Phase").eval::<String>().unwrap();
            if phase == "DONE" {
                break;
            }
            let published = left.load("return MP_OBSERVE()").eval::<bool>().unwrap();
            assert!(!right.load("return MP_OBSERVE()").eval::<bool>().unwrap());
            if !published {
                assert_eq!(
                    phase,
                    "DISPATCH",
                    "{}",
                    left.globals()
                        .get::<Option<String>>("PALACE_DIAG")
                        .unwrap()
                        .unwrap_or_default()
                );
                assert_eq!(left.load("return UI_MOVES").eval::<i64>().unwrap(), 1);
                right.load("WORK_X=16").exec().unwrap(); // replicated engine MOVE
                let saved = right.load(r#"
                  local cities={};for x,c in pairs(CITIES) do cities[x]={Owner=c.owner,ID=c.id,Properties=c.props} end
                  return JAC_CarrierMP.Encode({State=JAC_AbsorptionGameplay.GetState(),Cities=cities,Work=WORK_X,Transfers=TRANSFERS})
                "#).eval::<String>().unwrap();
                drop(right);
                right = palace_world(2, true);
                right.globals().set("PALACE_SAVED", saved).unwrap();
                right.load(r#"
                  local saved=JAC_CarrierMP.Decode(PALACE_SAVED)
                  for _,p in pairs(Players) do p.cities={} end
                  for x,record in pairs(saved.Cities) do
                    local c=CITIES[x];c.owner=record.Owner;c.id=record.ID;c.props=record.Properties
                    Players[c.owner].cities[x]=c
                  end
                  for _,plot in pairs(PLOTS) do if plot.city then plot.owner=CITIES[plot.city].owner end end
                  WORK_X=saved.Work;TRANSFERS=saved.Transfers
                  Game:SetProperty(JAC_AbsorptionGameplay.Rules.STATE_KEY,saved.State)
                "#).exec().unwrap();
                continue;
            }
            let raw = left.load("return MP_LAST()").eval::<String>().unwrap();
            for lua in [&right, &left] {
                assert!(
                    mp_receive(lua, &raw),
                    "step {step}/{phase}: {}",
                    lua.globals()
                        .get::<Option<String>>("PALACE_DIAG")
                        .unwrap()
                        .unwrap_or_default()
                );
                let transfers = lua.load("return TRANSFERS").eval::<i64>().unwrap();
                let _ = mp_receive(lua, &raw);
                assert_eq!(
                    lua.load("return TRANSFERS").eval::<i64>().unwrap(),
                    transfers
                );
            }
        }
        for lua in [&left, &right] {
            assert!(lua.load("return MP_PLAN()").eval::<bool>().unwrap());
            lua.load("assert(TRANSFERS==2 and CITIES[4].owner==1 and CITIES[16].owner==2);local c=JAC_AbsorptionGameplay.GetState().Transactions.checkpoint.NativeWorks[1].Carrier;assert(c.MP.Actor==1 and not c.MP.Loans[1].Issued and c.MP.Loans[2].ReturnIssued);assert(#JAC_AbsorptionGameplay.NativeWorkLocations(900)==1 and #c.MP.FinalActorObservation.Cities[1].State.UnitsOnCenter==1)").exec().unwrap();
        }
        assert_eq!(left.load("return UI_MOVES").eval::<i64>().unwrap(), 1);
        assert_eq!(right.load("return UI_MOVES").eval::<i64>().unwrap(), 0);
        let state = "return JAC_AbsorptionRules.Canonical(JAC_AbsorptionGameplay.GetState())";
        assert_eq!(
            left.load(state).eval::<String>().unwrap(),
            right.load(state).eval::<String>().unwrap()
        );
    }

    #[test]
    fn mp_palace_ai_owner_is_rejected_before_any_loan_or_move() {
        let lua = palace_world(2, false);
        assert!(lua.load("return MP_PLAN()").eval::<bool>().is_err());
        lua.load("assert(TRANSFERS==0 and UI_MOVES==0 and WORK_X==4)")
            .exec()
            .unwrap();
    }

    #[test]
    fn mp_archived_original_now_in_human_palace_recovers_through_public_entry() {
        let left = palace_world(1, true);
        let right = palace_world(2, true);
        for lua in [&left, &right] {
            lua.load(r#"
              GameEvents.JAC_NativeInventory_Operation={Add=function() end}
              TEST_STATE_EDIT(function(s)
                local source=JAC_AbsorptionRules.Copy(s.Transactions.checkpoint.NativeWorks[1].Source)
                source.Owner=3;source.CityID=16;source.X=16;source.Building=1;source.BuildingType='BUILDING_TEST'
                s.Archive['native-work:900']={ID='native-work:900',Kind='ITEM',Owner=-1,NativeOwnerMoves=2,NativeWork=source}
                s.Transactions={}
              end)
              function MP_PLAN()
                return JAC_AbsorptionGameplay.RecoverArchived('native-work:900',2,'recovery:palace').Status=='COMPLETE'
              end
              function MP_OBSERVE()
                return JAC_ObserveNativeCarrierMP(JAC_AbsorptionGameplay.GetState().Recoveries['recovery:palace'].NativeCarrier)
              end
            "#).exec().unwrap();
            lua.load(
                fs::read_to_string(
                    repo_root().join("poc/JAC_PoC_Absorption/Scripts/JAC_NativeInventory.lua"),
                )
                .unwrap(),
            )
            .exec()
            .unwrap();
            assert!(!lua.load("return MP_PLAN()").eval::<bool>().unwrap());
            lua.load("local s=JAC_AbsorptionGameplay.GetState();local r=s.Recoveries['recovery:palace'];assert(r.NativeSource.Owner==1 and r.NativeSource.Building==0 and r.NativeCarrier.MP.Actor==1)").exec().unwrap();
        }
        for _ in 0..12 {
            let phase = left.load("return JAC_AbsorptionGameplay.GetState().Recoveries['recovery:palace'].NativeCarrier.MP.Phase").eval::<String>().unwrap();
            if phase == "DONE" {
                break;
            }
            let published = left.load("return MP_OBSERVE()").eval::<bool>().unwrap();
            assert!(!right.load("return MP_OBSERVE()").eval::<bool>().unwrap());
            if !published {
                assert_eq!(phase, "DISPATCH");
                right.load("WORK_X=16").exec().unwrap();
                continue;
            }
            let raw = left.load("return MP_LAST()").eval::<String>().unwrap();
            for lua in [&right, &left] {
                assert!(mp_receive(lua, &raw));
            }
        }
        for lua in [&left, &right] {
            assert!(lua.load("return MP_PLAN()").eval::<bool>().unwrap());
            assert!(lua.load("return MP_PLAN()").eval::<bool>().unwrap());
            lua.load("assert(TRANSFERS==2 and CITIES[4].owner==1 and CITIES[16].owner==2 and WORK_X==16);local item=Players[2]:GetProperty('JAC_ABSORPTION_ASSETS_V1')['native-work:900'];assert(item.RecoveryToken=='recovery:palace' and item.NativeOwnerMoves==3 and item.NativeWork.Owner==2 and item.NativeWork.Index==900)").exec().unwrap();
        }
        assert_eq!(left.load("return UI_MOVES").eval::<i64>().unwrap(), 1);
        assert_eq!(right.load("return UI_MOVES").eval::<i64>().unwrap(), 0);
    }

    #[test]
    fn mp_palace_human_owner_waits_for_own_turn_and_rejects_recipient_sender() {
        let lua = palace_world(1, true);
        lua.load("Players[1].IsTurnActive=function() return false end;MP_PLAN();local c=JAC_AbsorptionGameplay.GetState().Transactions.checkpoint.NativeWorks[1].Carrier;assert(c.MP.Actor==1);assert(not MP_PLAN() and TRANSFERS==0 and not MP_OBSERVE());Players[1].IsTurnActive=function() return true end").exec().unwrap();
        let raw = mp_publish(&lua);
        lua.globals().set("PALACE_PACKET", raw.clone()).unwrap();
        lua.load("assert(not TEST_MP_RECEIVE(2,JAC_CarrierMP.Decode(PALACE_PACKET)));assert(TRANSFERS==0)").exec().unwrap();
        assert!(mp_receive(&lua, &raw));
    }

    fn extended_world(local: i64, mode: &str) -> Lua {
        let lua = mp_world(local, false);
        lua.globals().set("EXTENDED_MODE", mode).unwrap();
        lua.load(r#"
          CityManager.TransferCity(CITIES[16],3);CityManager.TransferCity(CITIES[0],3);TRANSFERS=0
          local cities=Players[3].GetCities
          Players[3].GetCities=function() local value=cities();value.GetCapitalCity=function() return CITIES[0] end;return value end
          TEST_STATE_EDIT(function(s)
            local snap=s.Transactions.checkpoint.NativeWorks[1];snap.Custodian=3;snap.Destination=3
            if EXTENDED_MODE=='scatter' then snap.Destination=-1 end
            if EXTENDED_MODE=='ai' then s.Transactions.checkpoint.To=3 end
            if EXTENDED_MODE=='recovery' then
              s.Recoveries={['recovery:test']={ID='native-work:900',Owner=3,Turn=CURRENT_TURN,
                Status='PREPARED',NativeSource=JAC_AbsorptionRules.Copy(snap.Source)}}
              s.Archive['native-work:900']={ID='native-work:900',Kind='ITEM',Owner=-1,
                NativeWork=JAC_AbsorptionRules.Copy(snap.Source),NativeOwnerMoves=1}
              s.Transactions={}
            end
          end)
          ExposedMembers.JAC_HeroReadAPI.FindCompatibleNativeSlots=function()
            return {{Owner=3,City=CITIES[16]:GetID(),X=16,Y=0,Building=1,Slot=0,SlotType='GREATWORKSLOT_RELIC'}} end
          function EXT_CARRIER()
            local s=JAC_AbsorptionGameplay.GetState()
            return EXTENDED_MODE=='recovery' and s.Recoveries['recovery:test'].NativeCarrier
              or s.Transactions.checkpoint.NativeWorks[1].Carrier
          end
          if EXTENDED_MODE=='recovery' then
            function MP_PLAN()
              return JAC_AbsorptionGameplay.RecoverArchived('native-work:900',3,'recovery:test').Status=='COMPLETE'
            end
          end
          function MP_OBSERVE() return JAC_ObserveNativeCarrierMP(EXT_CARRIER()) end
        "#).exec().unwrap();
        lua
    }

    fn artifact_world(local: i64, human: bool, recipient: i64) -> Lua {
        let lua = mp_world(local, human);
        lua.globals().set("ARTIFACT_RECIPIENT", recipient).unwrap();
        lua.load(r#"
          print=function(text) ARTIFACT_DIAG=text end
          Players[1].IsTurnActive=function() return HUMAN_DONOR end
          if ARTIFACT_RECIPIENT~=2 then
            CityManager.TransferCity(CITIES[16],3);CityManager.TransferCity(CITIES[0],3)
            local cities=Players[3].GetCities
            Players[3].GetCities=function() local out=cities();out.GetCapitalCity=function() return CITIES[0] end;return out end
          end
          TRANSFERS=0;OTHER_CREATOR={[901]=1,[902]=0,[903]=3}
          GameInfo.GreatWorks[4].GreatWorkObjectType='GREATWORKOBJECT_ARTIFACT'
          GameInfo.GreatWorkSlotTypes[1].GreatWorkSlotType='GREATWORKSLOT_ARTIFACT'
          GameInfo.GreatWork_ValidSubTypes=function()
            local done=false;return function() if not done then done=true;return {
              GreatWorkObjectType='GREATWORKOBJECT_ARTIFACT',GreatWorkSlotType='GREATWORKSLOT_ARTIFACT'} end end
          end
          Game.GetGreatWorkPlayer=function(index) return index==900 and 1 or assert(OTHER_CREATOR[index]) end
          Game.GetGreatWorkDataFromIndex=function(index) return {CreatorName='original '..Game.GetGreatWorkPlayer(index),TurnCreated=3} end
          for _,c in pairs(CITIES) do
            local old=c.GetBuildings
            c.GetBuildings=function(self)
              local b=old(self)
              b.GetNumGreatWorkSlots=function(_,building) return building==1 and 3 or 0 end
              b.GetGreatWorkInSlot=function(_,building,slot)
                if building~=1 then return -1 end
                if slot==0 and c.x==WORK_X then return 900 end
                if c.x==4 and slot>0 and slot<3 and LOST_OTHER~=900+slot
                  and not (slot==1 and RELOCATED_OTHER) then return 900+slot end
                if c.x==8 and slot==0 then return 903 end -- recipient-independent original
                if c.x==16 and slot==1 and (DUPLICATE_OTHER or RELOCATED_OTHER) then return 901 end
                if c.x==16 and slot==0 and OCCUPIED_TARGET then return 903 end
                return -1
              end
              b.GetGreatWorkTypeFromIndex=function() return 4 end
              b.GetCreatorNameFromIndex=function(_,index) return Game.GetGreatWorkDataFromIndex(index).CreatorName end
              b.GetTurnFromIndex=function() return 3 end
              return b
            end
          end
          TEST_STATE_EDIT(function(s)
            local tx=s.Transactions.checkpoint;local snap=tx.NativeWorks[1]
            snap.Source=JAC_AbsorptionGameplay.NativeWorkLocations(900)[1]
            snap.Source.BuildingType='BUILDING_TEST'
            snap.Custodian=ARTIFACT_RECIPIENT==2 and 2 or 3;snap.Destination=ARTIFACT_RECIPIENT
            tx.To=ARTIFACT_RECIPIENT==3 and 3 or 2
          end)
          ExposedMembers.JAC_HeroReadAPI.FindCompatibleNativeSlots=function()
            return {{Owner=ARTIFACT_RECIPIENT==2 and 2 or 3,City=CITIES[16].id,X=16,Y=0,
              Building=1,Slot=0,SlotType='GREATWORKSLOT_ARTIFACT'}} end
          UI.RequestPlayerOperation=function(actor,op,p)
            local state=JAC_AbsorptionGameplay.GetState()
            local c=ARTIFACT_RECOVERY and state.Recoveries['recovery:artifact'].NativeCarrier
              or state.Transactions.checkpoint.NativeWorks[1].Carrier
            assert(actor==LOCAL_PLAYER and actor==c.MP.Actor and Players[actor]:IsHuman())
            if op=='script' then PACKETS[#PACKETS+1]=p;return true end
            assert(op=='move' and p.owner==actor and p.index==900 and p.slot==0)
            assert(CITIES[4].owner==actor and CITIES[16].owner==actor)
            assert(p.source==CITIES[4].id and p.target==CITIES[16].id)
            assert(CITIES[16]:GetBuildings():GetGreatWorkInSlot(1,0)==-1,'occupied target must never swap')
            UI_MOVES=UI_MOVES+1
            if UNKNOWN_MOVE then return nil end
            WORK_X=16;return true
          end
          function MP_RECEIVE(raw)
            local get,reads=Game.GetLocalPlayer,ExposedMembers.JAC_HeroReadAPI
            Game.GetLocalPlayer=function() error('Gameplay local player fallback') end
            ExposedMembers.JAC_HeroReadAPI=nil
            local p=JAC_CarrierMP.Decode(raw)
            local ok=TEST_MP_RECEIVE(p.Actor,p)
            Game.GetLocalPlayer,ExposedMembers.JAC_HeroReadAPI=get,reads
            return ok
          end
        "#).exec().unwrap();
        lua
    }

    fn artifact_saved(lua: &Lua) -> String {
        lua.load(r#"
          local cities={};for x,c in pairs(CITIES) do cities[x]={Owner=c.owner,ID=c.id,Properties=c.props} end
          return JAC_CarrierMP.Encode({State=JAC_AbsorptionGameplay.GetState(),Cities=cities,
            Work=WORK_X,Transfers=TRANSFERS,Creators=OTHER_CREATOR})
        "#).eval().unwrap()
    }

    fn artifact_loaded(local: i64, human: bool, recipient: i64, saved: &str) -> Lua {
        let lua = artifact_world(local, human, recipient);
        lua.globals().set("ARTIFACT_SAVED", saved).unwrap();
        lua.load(
            r#"
          local saved=JAC_CarrierMP.Decode(ARTIFACT_SAVED)
          for _,p in pairs(Players) do p.cities={} end
          for x,record in pairs(saved.Cities) do
            local c=CITIES[x];c.owner=record.Owner;c.id=record.ID;c.props=record.Properties
            Players[c.owner].cities[x]=c
          end
          for _,plot in pairs(PLOTS) do if plot.city then plot.owner=CITIES[plot.city].owner end end
          WORK_X=saved.Work;TRANSFERS=saved.Transfers;OTHER_CREATOR=saved.Creators
          Game:SetProperty(JAC_AbsorptionGameplay.Rules.STATE_KEY,saved.State)
        "#,
        )
        .exec()
        .unwrap();
        lua
    }

    #[test]
    fn mp_artifact_full_source_empty_target_preserves_originals_and_replays_in_two_vms() {
        for human in [false, true] {
            for recipient in [2, 3, -1] {
                let actor = if human && recipient == 3 { 1 } else { 2 };
                let left = artifact_world(actor, human, recipient);
                let mut right = artifact_world(3, human, recipient);
                for lua in [&left, &right] {
                    lua.load("MP_PLAN()").exec().unwrap();
                }
                for _ in 0..8 {
                    let current=left.load("return JAC_AbsorptionGameplay.GetState().Transactions.checkpoint.NativeWorks[1].Carrier.MP.Phase").eval::<String>().unwrap();
                    if current == "DONE" {
                        break;
                    }
                    if current == "DISPATCH" {
                        assert!(!left.load("return MP_OBSERVE()").eval::<bool>().unwrap());
                        right.load("WORK_X=16").exec().unwrap();
                        right = artifact_loaded(3, human, recipient, &artifact_saved(&right));
                    }
                    assert!(left.load("return MP_OBSERVE()").eval::<bool>().unwrap(),
                        "artifact phase={current} human={human} recipient={recipient} actor={actor}: {}",
                        left.load("return tostring(ARTIFACT_DIAG)").eval::<String>().unwrap());
                    let raw = left.load("return MP_LAST()").eval::<String>().unwrap();
                    for lua in [&left, &right] {
                        assert!(
                            mp_receive(lua, &raw),
                            "{current}: {}",
                            lua.load("return tostring(ARTIFACT_DIAG)")
                                .eval::<String>()
                                .unwrap()
                        );
                        let state="return JAC_AbsorptionRules.Canonical(JAC_AbsorptionGameplay.GetState())";
                        let before = lua.load(state).eval::<String>().unwrap();
                        let _ = mp_receive(lua, &raw); // stale SELECT may reject; no second effect
                        assert_eq!(before, lua.load(state).eval::<String>().unwrap());
                    }
                }
                for lua in [&left, &right] {
                    assert!(lua.load("return MP_PLAN()").eval::<bool>().unwrap());
                    lua.load(r#"
                      local c=JAC_AbsorptionGameplay.GetState().Transactions.checkpoint.NativeWorks[1].Carrier
                      assert(c.MP.Phase=='DONE' and WORK_X==16 and CITIES[4].owner==1 and CITIES[16].owner==c.Owner)
                      for slot,index in ipairs({901,902}) do assert(CITIES[4]:GetBuildings():GetGreatWorkInSlot(1,slot)==index) end
                      assert(#JAC_AbsorptionGameplay.NativeWorkLocations(900)==1)
                      assert(#JAC_AbsorptionGameplay.NativeWorkLocations(901)==1 and OTHER_CREATOR[901]==1)
                      assert(#JAC_AbsorptionGameplay.NativeWorkLocations(902)==1 and OTHER_CREATOR[902]==0)
                      assert(#JAC_AbsorptionGameplay.NativeWorkLocations(903)==1)
                    "#).exec().unwrap();
                }
                assert_eq!(left.load("return UI_MOVES").eval::<i64>().unwrap(), 1);
                assert_eq!(right.load("return UI_MOVES").eval::<i64>().unwrap(), 0);
                let state =
                    "return JAC_AbsorptionRules.Canonical(JAC_AbsorptionGameplay.GetState())";
                assert_eq!(
                    left.load(state).eval::<String>().unwrap(),
                    right.load(state).eval::<String>().unwrap()
                );
            }
        }
    }

    #[test]
    fn mp_artifact_public_recovery_moves_once_and_preserves_museum_originals() {
        for recipient in [2, 3] {
            let left = artifact_world(2, false, recipient);
            let right = artifact_world(3, false, recipient);
            for lua in [&left, &right] {
                lua.load(r#"
                  ARTIFACT_RECOVERY=true
                  GameInfo.GreatWorks.GREATWORK_TEST=GameInfo.GreatWorks[4]
                  GameEvents.JAC_NativeInventory_Operation={Add=function() end}
                  TEST_STATE_EDIT(function(s)
                    s.Archive['native-work:900']={ID='native-work:900',Kind='ITEM',Owner=-1,NativeOwnerMoves=1,
                      NativeWork=JAC_AbsorptionRules.Copy(s.Transactions.checkpoint.NativeWorks[1].Source)}
                  end)
                  function MP_PLAN()
                    return JAC_AbsorptionGameplay.RecoverArchived('native-work:900',ARTIFACT_RECIPIENT,'recovery:artifact').Status=='COMPLETE'
                  end
                  function MP_OBSERVE()
                    return JAC_ObserveNativeCarrierMP(JAC_AbsorptionGameplay.GetState().Recoveries['recovery:artifact'].NativeCarrier)
                  end
                "#).exec().unwrap();
                lua.load(
                    fs::read_to_string(
                        repo_root().join("poc/JAC_PoC_Absorption/Scripts/JAC_NativeInventory.lua"),
                    )
                    .unwrap(),
                )
                .exec()
                .unwrap();
                assert!(!lua.load("return MP_PLAN()").eval::<bool>().unwrap());
            }
            for _ in 0..12 {
                let phase=left.load("return JAC_AbsorptionGameplay.GetState().Recoveries['recovery:artifact'].NativeCarrier.MP.Phase").eval::<String>().unwrap();
                if phase == "DONE" {
                    break;
                }
                if phase == "DISPATCH" {
                    assert!(!left.load("return MP_OBSERVE()").eval::<bool>().unwrap());
                    right.load("WORK_X=16").exec().unwrap();
                }
                let packet = mp_publish(&left);
                for lua in [&right, &left] {
                    assert!(mp_receive(lua, &packet));
                }
            }
            for lua in [&left, &right] {
                assert!(lua.load("return MP_PLAN()").eval::<bool>().unwrap());
                assert!(lua.load("return MP_PLAN()").eval::<bool>().unwrap());
                lua.load("local r=Players[ARTIFACT_RECIPIENT]:GetProperty('JAC_ABSORPTION_ASSETS_V1')['native-work:900'];assert(r.NativeOwnerMoves==2 and r.NativeWork.Index==900 and r.NativeWork.Owner==ARTIFACT_RECIPIENT);for _,index in ipairs({900,901,902,903}) do assert(#JAC_AbsorptionGameplay.NativeWorkLocations(index)==1) end;assert(CITIES[4].owner==1 and CITIES[16].owner==ARTIFACT_RECIPIENT and WORK_X==16)").exec().unwrap();
            }
            assert_eq!(left.load("return UI_MOVES").eval::<i64>().unwrap(), 1);
            assert_eq!(right.load("return UI_MOVES").eval::<i64>().unwrap(), 0);
        }
    }

    #[test]
    fn mp_artifact_unknown_send_rejoin_never_repeats_or_returns_loan() {
        let left = artifact_world(2, false, 3);
        left.load("MP_PLAN()").exec().unwrap();
        mp_until(&left, "DISPATCH");
        left.load("UNKNOWN_MOVE=true").exec().unwrap();
        assert!(!left.load("return MP_OBSERVE()").eval::<bool>().unwrap());
        assert_eq!(left.load("return UI_MOVES").eval::<i64>().unwrap(), 1);
        let rejoin = artifact_loaded(2, false, 3, &artifact_saved(&left));
        for _ in 0..3 {
            let raw = mp_publish(&rejoin);
            assert!(mp_receive(&rejoin, &raw));
        }
        rejoin.load(r#"
          local c=JAC_AbsorptionGameplay.GetState().Transactions.checkpoint.NativeWorks[1].Carrier
          assert(c.MP.Phase=='DISPATCH' and c.MP.WaitingFor=='NATIVE_QUEUE_RESOLUTION')
          assert(WORK_X==4 and TRANSFERS==2 and CITIES[4].owner==2 and CITIES[16].owner==2 and UI_MOVES==0)
        "#).exec().unwrap();
    }

    #[test]
    fn mp_artifact_occupied_target_and_unqualified_sender_never_send_move() {
        let lua = artifact_world(2, false, 2);
        lua.load("MP_PLAN()").exec().unwrap();
        mp_until(&lua, "PREPARE");
        let raw = mp_publish(&lua);
        lua.globals().set("ARTIFACT_PACKET", raw.as_str()).unwrap();
        lua.load("assert(not TEST_MP_RECEIVE(3,JAC_CarrierMP.Decode(ARTIFACT_PACKET)));assert(TRANSFERS==0)").exec().unwrap();
        assert!(mp_receive(&lua, &raw));
        mp_until(&lua, "DISPATCH");
        // A slot changed after CLAIM. The actor must not enqueue a swap.
        lua.load("OCCUPIED_TARGET=true").exec().unwrap();
        assert!(!lua.load("return MP_OBSERVE()").eval::<bool>().unwrap());
        let raw = mp_publish(&lua);
        assert!(!mp_receive(&lua, &raw));
        lua.load("assert(UI_MOVES==0 and WORK_X==4 and TRANSFERS==1)")
            .exec()
            .unwrap();
    }

    #[test]
    fn mp_artifact_partial_observation_and_changed_source_never_borrow() {
        for corruption in ["p.Payload=p.Payload:sub(1,#p.Payload-1)",
            "local o=JAC_CarrierMP.Decode(p.Payload);o.Locations[1].CreatedByPlayer=3;o.ArtifactWorks[900]=o.Locations;p.Payload=JAC_CarrierMP.Encode(o)"] {
            let lua=artifact_world(2,false,2);
            lua.load("MP_PLAN()").exec().unwrap();mp_until(&lua,"PREPARE");
            let raw=mp_publish(&lua);lua.globals().set("ARTIFACT_PACKET",raw).unwrap();
            let bad=lua.load(format!("local p=JAC_CarrierMP.Decode(ARTIFACT_PACKET);{corruption};return JAC_CarrierMP.Encode(p)")).eval::<String>().unwrap();
            assert!(!mp_receive(&lua,&bad));
            lua.load("assert(TRANSFERS==0 and UI_MOVES==0 and WORK_X==4)").exec().unwrap();
        }
    }

    #[test]
    fn mp_artifact_other_original_loss_relocation_duplicate_or_provenance_blocks_return() {
        for corruption in [
            "LOST_OTHER=901",
            "OTHER_CREATOR[901]=3",
            "DUPLICATE_OTHER=true",
            "RELOCATED_OTHER=true",
            "TEST_STATE_EDIT(function(s) s.Transactions.checkpoint.NativeWorks[1].Carrier.MP.ArtifactOriginals=nil end)",
        ] {
            let lua = artifact_world(2, false, 3);
            lua.load("MP_PLAN()").exec().unwrap();
            mp_until(&lua, "DISPATCH");
            assert!(!lua.load("return MP_OBSERVE()").eval::<bool>().unwrap());
            lua.load(corruption).exec().unwrap();
            let published=lua.load("return MP_OBSERVE()").eval::<bool>().unwrap();
            let raw=if published {Some(lua.load("return MP_LAST()").eval::<String>().unwrap())} else {None};
            if let Some(raw)=&raw {assert!(!mp_receive(&lua,raw),"{corruption}");}
            else {assert!(lua.load("return tostring(ARTIFACT_DIAG):find('multiple source slots',1,true)~=nil").eval::<bool>().unwrap(),"{corruption}: {}",lua.load("return tostring(ARTIFACT_DIAG)").eval::<String>().unwrap());}
            let rejoin = artifact_loaded(2, false, 3, &artifact_saved(&lua));
            rejoin.load(corruption).exec().unwrap();
            if let Some(raw)=&raw {assert!(!mp_receive(&rejoin,raw));}
            else {assert!(!rejoin.load("return MP_OBSERVE()").eval::<bool>().unwrap());}
            rejoin.load(r#"
              local c=JAC_AbsorptionGameplay.GetState().Transactions.checkpoint.NativeWorks[1].Carrier
              assert(c.MP.Phase=='DISPATCH' and CITIES[4].owner==2 and CITIES[16].owner==2)
              assert(TRANSFERS==2 and UI_MOVES==0 and WORK_X==16)
            "#).exec().unwrap();
        }
    }

    #[test]
    fn mp_extended_scatter_ai_and_recovery_use_one_human_move_in_two_vms() {
        for mode in ["scatter", "ai", "recovery"] {
            let left = extended_world(2, mode);
            let right = extended_world(3, mode);
            for lua in [&left, &right] {
                assert!(
                    !lua.load("return MP_PLAN()").eval::<bool>().unwrap(),
                    "{mode}"
                );
            }
            for _ in 0..4 {
                let packet = mp_publish(&left);
                for lua in [&left, &right] {
                    assert!(mp_receive(lua, &packet), "{mode}");
                    let before = lua.load("return TRANSFERS").eval::<i64>().unwrap();
                    let _ = mp_receive(lua, &packet);
                    assert_eq!(lua.load("return TRANSFERS").eval::<i64>().unwrap(), before);
                }
            }
            left.load("MP_OBSERVE()").exec().unwrap();
            assert_eq!(left.load("return UI_MOVES").eval::<i64>().unwrap(), 1);
            right.load("WORK_X=16").exec().unwrap();
            for _ in 0..2 {
                let packet = mp_publish(&left);
                for lua in [&left, &right] {
                    assert!(mp_receive(lua, &packet), "{mode}");
                    assert!(mp_receive(lua, &packet), "{mode}");
                }
            }
            for lua in [&left, &right] {
                assert!(
                    lua.load("return MP_PLAN()").eval::<bool>().unwrap(),
                    "{mode}"
                );
                lua.load(
                    r#"
                  local c=EXT_CARRIER()
                  assert(c.MP.Actor==2 and c.Owner==3 and c.MP.Phase=='DONE')
                  assert(CITIES[4]:GetOwner()==1 and CITIES[16]:GetOwner()==3 and WORK_X==16)
                  assert(TRANSFERS==4 and CITIES[4]:GetProperty('MP_TEST_PROPERTY').Value=='復元')
                "#,
                )
                .exec()
                .unwrap();
            }
            let state = "return JAC_AbsorptionRules.Canonical(JAC_AbsorptionGameplay.GetState())";
            assert_eq!(
                left.load(state).eval::<String>().unwrap(),
                right.load(state).eval::<String>().unwrap()
            );
        }
    }

    #[test]
    fn mp_recovery_token_collision_and_other_context_city_reservation_are_rejected() {
        let lua = extended_world(2, "recovery");
        lua.load("MP_PLAN()").exec().unwrap();
        let select = mp_publish(&lua);
        lua.load(
            r#"
          TEST_STATE_EDIT(function(s)
            local c=JAC_AbsorptionRules.Copy(s.Recoveries['recovery:test'].NativeCarrier)
            s.Transactions.collision={Turn=CURRENT_TURN,NativeWorks={{Carrier=c}}}
          end)
        "#,
        )
        .exec()
        .unwrap();
        assert!(!mp_receive(&lua, &select));
        lua.load("assert(TRANSFERS==0 and UI_MOVES==0)")
            .exec()
            .unwrap();
        lua.load(
            r#"
          TEST_STATE_EDIT(function(s)
            s.Transactions.collision.NativeWorks[1].Carrier.Token='another'
            s.Transactions.collision.NativeWorks[1].Carrier.Index=901
            s.Transactions.collision.NativeWorks[1].Carrier.MP.Phase='PREPARE'
            s.Transactions.collision.NativeWorks[1].Carrier.MP.Loans={{X=16,Y=0}}
          end)
        "#,
        )
        .exec()
        .unwrap();
        assert!(!mp_receive(&lua, &select));
        lua.load("assert(TRANSFERS==0 and EXT_CARRIER().MP.WaitingFor=='TARGET_SLOT')")
            .exec()
            .unwrap();
        lua.load(r#"
          TEST_STATE_EDIT(function(s)
            s.Recoveries.other={Status='WAITING_UI',NativeCarrier=s.Transactions.collision.NativeWorks[1].Carrier}
            s.Transactions={}
          end)
        "#).exec().unwrap();
        assert!(!mp_receive(&lua, &select));
        lua.load("assert(TRANSFERS==0 and EXT_CARRIER().MP.WaitingFor=='TARGET_SLOT')")
            .exec()
            .unwrap();
        lua.load(
            r#"
          TEST_STATE_EDIT(function(s)
            s.Recoveries.other.NativeCarrier.Index=900
            s.Recoveries.other.NativeCarrier.MP.Loans={{X=0,Y=0}}
          end)
        "#,
        )
        .exec()
        .unwrap();
        assert!(!mp_receive(&lua, &select));
        lua.load("assert(TRANSFERS==0 and EXT_CARRIER().MP.WaitingFor=='TARGET_SLOT')")
            .exec()
            .unwrap();
        lua.load("TEST_STATE_EDIT(function(s) s.Recoveries.other.Status='COMPLETE' end)")
            .exec()
            .unwrap();
        assert!(mp_receive(&lua, &select));
        lua.load("assert(TRANSFERS==0 and EXT_CARRIER().MP.Phase=='PREPARE')")
            .exec()
            .unwrap();
    }

    #[test]
    fn mp_extended_rejoin_preserves_claim_without_reissuing_an_unknown_move() {
        for mode in ["scatter", "ai", "recovery"] {
            let first = extended_world(2, mode);
            first.load("MP_PLAN()").exec().unwrap();
            for _ in 0..4 {
                assert!(mp_receive(&first, &mp_publish(&first)));
            }
            let saved=first.load(r#"
              local cities={};for x,c in pairs(CITIES) do cities[x]={Owner=c.owner,ID=c.id,Properties=c.props} end
              return JAC_CarrierMP.Encode({State=JAC_AbsorptionGameplay.GetState(),Cities=cities,Work=WORK_X,Transfers=TRANSFERS})
            "#).eval::<String>().unwrap();
            drop(first);
            let resumed = extended_world(2, mode);
            resumed.globals().set("SAVED_EXT", saved).unwrap();
            resumed.load(r#"
              local saved=JAC_CarrierMP.Decode(SAVED_EXT)
              for _,p in pairs(Players) do p.cities={} end
              for x,r in pairs(saved.Cities) do
                local c=CITIES[x];c.owner=r.Owner;c.id=r.ID;c.props=r.Properties;Players[c.owner].cities[x]=c
              end
              for _,plot in pairs(PLOTS) do if plot.city then plot.owner=CITIES[plot.city].owner end end
              WORK_X=saved.Work;TRANSFERS=saved.Transfers
              Game:SetProperty(JAC_AbsorptionGameplay.Rules.STATE_KEY,saved.State)
            "#).exec().unwrap();
            for _ in 0..3 {
                assert!(mp_receive(&resumed, &mp_publish(&resumed)));
            }
            resumed.load("assert(UI_MOVES==0 and TRANSFERS==2 and WORK_X==4 and EXT_CARRIER().MP.WaitingFor=='NATIVE_QUEUE_RESOLUTION')").exec().unwrap();
            resumed.load("WORK_X=16").exec().unwrap(); // delayed engine publication
            for _ in 0..2 {
                assert!(mp_receive(&resumed, &mp_publish(&resumed)));
            }
            assert!(resumed.load("return MP_PLAN()").eval::<bool>().unwrap());
            resumed
                .load("assert(UI_MOVES==0 and TRANSFERS==4 and CITIES[16]:GetOwner()==3)")
                .exec()
                .unwrap();
        }
    }

    #[test]
    fn mp_extended_receives_only_selected_human_and_waits_without_an_executor() {
        let lua = extended_world(2, "ai");
        lua.load("Players[2].IsHuman=function() return false end")
            .exec()
            .unwrap();
        assert!(!lua.load("return MP_PLAN()").eval::<bool>().unwrap());
        lua.load("assert(TRANSFERS==0 and EXT_CARRIER()==nil and JAC_AbsorptionGameplay.GetState().Transactions.checkpoint.WaitingFor=='MP_HUMAN_EXECUTOR')").exec().unwrap();
        lua.load("Players[2].IsHuman=function() return true end;MP_PLAN()")
            .exec()
            .unwrap();
        let raw = mp_publish(&lua);
        lua.globals().set("AUTH_PACKET", raw.clone()).unwrap();
        assert!(!lua
            .load("local p=JAC_CarrierMP.Decode(AUTH_PACKET);p.Actor=3;return TEST_MP_RECEIVE(3,p)")
            .eval::<bool>()
            .unwrap());
        assert!(!lua
            .load("local p=JAC_CarrierMP.Decode(AUTH_PACKET);return TEST_MP_RECEIVE(1,p)")
            .eval::<bool>()
            .unwrap());
        assert!(mp_receive(&lua, &raw));
        lua.load("assert(EXT_CARRIER().MP.Phase=='PREPARE' and TRANSFERS==0 and UI_MOVES==0)")
            .exec()
            .unwrap();
    }

    #[test]
    fn mp_extended_executor_selection_is_independent_of_peer_turn_activity() {
        let left = extended_world(2, "ai");
        let right = extended_world(3, "ai");
        for lua in [&left, &right] {
            lua.load("Players[1].IsHuman=function() return true end;MP_PLAN()")
                .exec()
                .unwrap();
        }
        right
            .load("Players[1].IsTurnActive=function() return false end")
            .exec()
            .unwrap();
        // Recreate the preloan intent under different activity observations.
        for lua in [&left, &right] {
            lua.load("TEST_STATE_EDIT(function(s) s.Transactions.checkpoint.NativeWorks[1].Carrier=nil end);MP_PLAN();assert(EXT_CARRIER().MP.Actor==1 and TRANSFERS==0)").exec().unwrap();
        }
        let state = "return JAC_AbsorptionRules.Canonical(JAC_AbsorptionGameplay.GetState())";
        assert_eq!(
            left.load(state).eval::<String>().unwrap(),
            right.load(state).eval::<String>().unwrap()
        );
    }

    #[test]
    fn mp_full_absorption_return_verifies_after_source_city_inheritance_and_next_turn() {
        let lua = extended_world(2, "scatter");
        lua.load(r#"
          GameEvents.JAC_NativeInventory_Operation={Add=function() end}
          GameInfo.GreatWorks.GREATWORK_TEST=GameInfo.GreatWorks[4]
          Players[1].units={}
          Players[1]:SetProperty('JAC_ABSORPTION_ASSETS_V1',{})
          local sourceCities=Players[1].GetCities
          Players[1].GetCities=function()
            local collection=sourceCities()
            collection.GetCount=function() local n=0;for _ in pairs(Players[1].cities) do n=n+1 end;return n end
            return collection
          end
          TEST_STATE_EDIT(function(s) s.Transactions={} end)
          DealManager={EnactWorkingDeal=function() error('unexpected deal') end,
            GetWorkingDeal=function() error('unexpected deal') end,
            GetPossibleDealItems=function() error('unexpected deal') end}
          GameInfo.JAC_AbsorptionOutcomes=function()
            local rows={{Kind='GREAT_PERSON',Outcome='RECRUIT',Destination='ABSORBER'},
              {Kind='ITEM',Outcome='RETURN',Destination='RETURN'}};local i=0
            return function() i=i+1;return rows[i] end
          end
          PLAN={ID='full-native-return',From=1,To=2,Mode='MERGER',EntryConditionsMet=true,
            RelationsResolved=true,Leader='RECRUIT',Units={},Assets={},NativeWorks={[900]={Outcome='RETURN',ReturnOwner=3}}}
          function MP_OBSERVE()
            return JAC_ObserveNativeCarrierMP(JAC_AbsorptionGameplay.GetState().Transactions[PLAN.ID].NativeWorks[1].Carrier)
          end
        "#).exec().unwrap();
        lua.load(transport::inventory_binding()).exec().unwrap();
        lua.load("JAC_AbsorptionGameplay.Apply(PLAN)")
            .exec()
            .unwrap();
        for _ in 0..12 {
            if lua.load("return JAC_AbsorptionGameplay.GetState().Transactions[PLAN.ID].Status=='COMPLETE'").eval::<bool>().unwrap() { break; }
            if lua.load("return MP_OBSERVE()").eval::<bool>().unwrap() {
                assert!(mp_receive(
                    &lua,
                    &lua.load("return MP_LAST()").eval::<String>().unwrap()
                ));
            }
            lua.load("JAC_AbsorptionGameplay.Resume(PLAN.ID)")
                .exec()
                .unwrap();
        }
        lua.load(r#"
          local tx=JAC_AbsorptionGameplay.GetState().Transactions[PLAN.ID]
          assert(tx.Status=='COMPLETE' and TRANSFERS==6 and UI_MOVES==1)
          assert(CITIES[4]:GetOwner()==2 and CITIES[8]:GetOwner()==2 and CITIES[16]:GetOwner()==3 and WORK_X==16)
          CURRENT_TURN=CURRENT_TURN+1
          assert(JAC_AbsorptionGameplay.Verify(PLAN.ID,false))
          assert(TRANSFERS==6 and UI_MOVES==1)
        "#).exec().unwrap();
    }

    #[test]
    fn mp_codec_accepts_a_large_world_within_the_wire_byte_limit() {
        let left = mp_world(2, false);
        let right = mp_world(3, false);
        let payload=left.load(r#"
          local world={Cities={},Works={}}
          for id=1,80 do
            local city={Owner=id%22,ID=id,Buildings={},Properties={},SLOTS={},WORK={}}
            for building=1,20 do
              city.Buildings[building]=true;city.SLOTS[building]=3;city.WORK[building]={[0]=-1,[1]=-1,[2]=-1}
              city.Properties['property:'..building]={Value=id+building,Registered=true}
            end
            world.Cities[id]=city
            world.Works[id]={Index=id,Owner=id%22,CityID=id,CreatorName='original creator',Effects={'TOURISM 4'}}
          end
          return JAC_CarrierMP.Encode(world)
        "#).eval::<String>().unwrap();
        assert!(payload.len() < 1048576);
        right.globals().set("LARGE_WORLD", payload.clone()).unwrap();
        let roundtrip = right
            .load("return JAC_CarrierMP.Encode(JAC_CarrierMP.Decode(LARGE_WORLD))")
            .eval::<String>()
            .unwrap();
        assert_eq!(roundtrip, payload);
    }

    #[test]
    fn mp_codec_round_trips_finite_double_bits_between_independent_vms() {
        let sender = ready();
        let receiver = ready();
        let encode = sender
            .load("return JAC_CarrierMP.Encode")
            .eval::<Function>()
            .unwrap();
        let decode = receiver
            .load("return JAC_CarrierMP.Decode")
            .eval::<Function>()
            .unwrap();
        let mut values = vec![
            0.0,
            -0.0,
            1.0000000000000002,
            123456789012345.67,
            f64::MIN_POSITIVE,
            f64::MAX,
            f64::from_bits(1),
            f64::from_bits(0x000f_ffff_ffff_ffff),
        ];
        // Deterministic bit patterns exercise signs, exponents, and mantissas;
        // avoid testing only decimal values exactly representable in 14 digits.
        let mut bits = 0x19ad_66db_3479_d591u64;
        for _ in 0..2048 {
            bits = bits.wrapping_mul(6364136223846793005).wrapping_add(1);
            let value = f64::from_bits(bits);
            if value.is_finite() {
                values.push(value);
            }
        }
        for value in values {
            let wire = encode.call::<String>(value).unwrap();
            let restored = decode.call::<f64>(wire.clone()).unwrap();
            assert_eq!(restored.to_bits(), value.to_bits(), "{value:?}: {wire}");
        }
    }

    #[test]
    fn mp_codec_preserves_numeric_and_string_keys_and_nearby_double_keys() {
        let sender = ready();
        let receiver = ready();
        let wire = sender
            .load(
                r#"
          return JAC_CarrierMP.Encode({[1]='number',['1']='string',
            [1.0000000000000002]='next',[1.0000000000000004]='next-next',
            Culture=123456789012345.67,Accumulation=1.0000000000000002})
        "#,
            )
            .eval::<String>()
            .unwrap();
        receiver.globals().set("CODEC_WIRE", wire).unwrap();
        receiver
            .load(
                r#"
          local v=JAC_CarrierMP.Decode(CODEC_WIRE)
          assert(v[1]=='number' and v['1']=='string')
          assert(v[1.0000000000000002]=='next' and v[1.0000000000000004]=='next-next')
          assert(v.Culture==123456789012345.67 and v.Accumulation==1.0000000000000002)
          local n=0;for _ in pairs(v) do n=n+1 end;assert(n==6)
        "#,
            )
            .exec()
            .unwrap();
    }

    #[test]
    fn mp_codec_rejects_nonfinite_values_and_keys_in_both_directions() {
        let lua = ready();
        lua.load(
            r#"
          for _,v in ipairs({math.huge,-math.huge,0/0}) do
            assert(not pcall(JAC_CarrierMP.Encode,v))
            assert(not pcall(JAC_CarrierMP.Encode,{Culture=v}))
          end
          assert(not pcall(JAC_CarrierMP.Encode,{[math.huge]='invalid-key'}))
          for _,s in ipairs({'inf','-inf','nan','1e999'}) do
            assert(not pcall(JAC_CarrierMP.Decode,'n'..#s..':'..s))
          end
        "#,
        )
        .exec()
        .unwrap();
    }

    #[test]
    fn mp_borrowed_city_observation_rejects_a_single_bit_double_change() {
        let lua = mp_world(2, false);
        lua.load("MP_PLAN()").exec().unwrap();
        for _ in 0..2 {
            let raw = mp_publish(&lua);
            assert!(mp_receive(&lua, &raw));
        }
        let raw = mp_publish(&lua);
        lua.globals().set("MP_RAW", raw).unwrap();
        let bad = lua
            .load(
                r#"
          local p=JAC_CarrierMP.Decode(MP_RAW);local o=JAC_CarrierMP.Decode(p.Payload)
          local before=o.Cities[1].State.Loyalty
          o.Cities[1].State.Loyalty=before-2^-46
          assert(o.Cities[1].State.Loyalty~=before)
          assert(tostring(o.Cities[1].State.Loyalty)==tostring(before))
          p.Payload=JAC_CarrierMP.Encode(o);return JAC_CarrierMP.Encode(p)
        "#,
            )
            .eval::<String>()
            .unwrap();
        assert!(!mp_receive(&lua, &bad));
        lua.load("assert(UI_MOVES==0 and TRANSFERS==1)")
            .exec()
            .unwrap();
        assert!(!mp_receive(&lua, &bad));
        assert!(!mp_receive(&lua, &bad));
        lua.load("assert(UI_MOVES==0 and TRANSFERS==2 and CITIES[4]:GetOwner()==1)")
            .exec()
            .unwrap();
        assert!(!mp_receive(
            &lua,
            &lua.globals().get::<String>("MP_RAW").unwrap()
        ));
        assert!(mp_receive(&lua, &mp_publish(&lua)));
    }

    #[test]
    fn mp_interrupted_borrow_and_return_resume_from_saved_intents() {
        let lua = mp_world(2, false);
        lua.load("MP_PLAN()").exec().unwrap();
        let selection = mp_publish(&lua);
        assert!(mp_receive(&lua, &selection));
        let prepare = mp_publish(&lua);
        lua.load("FAIL_CITY=true").exec().unwrap();
        assert!(mp_receive(&lua, &prepare));
        lua.load("FAIL_CITY=false").exec().unwrap();
        let rollback_report = mp_publish(&lua);
        assert!(mp_receive(&lua, &rollback_report));
        let resume = mp_publish(&lua);
        assert!(mp_receive(&lua, &resume));
        mp_until(&lua, "DISPATCH");
        assert!(!lua.load("return MP_OBSERVE()").eval::<bool>().unwrap());
        let moved = mp_publish(&lua);
        lua.load("FAIL_CITY=true").exec().unwrap();
        assert!(mp_receive(&lua, &moved));
        lua.load("FAIL_CITY=false").exec().unwrap();
        let resume = mp_publish(&lua);
        assert!(mp_receive(&lua, &resume));
        let restored = mp_publish(&lua);
        assert!(mp_receive(&lua, &restored));
        assert!(lua.load("return MP_PLAN()").eval::<bool>().unwrap());
        lua.load("assert(UI_MOVES==1 and TRANSFERS==2 and CITIES[4]:GetOwner()==1)")
            .exec()
            .unwrap();
    }

    #[test]
    fn mp_reloaded_ui_does_not_reissue_an_authorized_move() {
        let lua = mp_world(2, false);
        assert!(!lua.load("return MP_PLAN()").eval::<bool>().unwrap());
        mp_until(&lua, "DISPATCH");
        // Reconstruct the UI context with a new session, retaining Gameplay's
        // saved grant. An unknown dispatch must not produce another MOVE.
        lua.load(transport::ui_binding()).exec().unwrap();
        for _ in 0..3 {
            let raw = mp_publish(&lua);
            assert!(mp_receive(&lua, &raw));
        }
        assert_eq!(lua.load("return UI_MOVES").eval::<i64>().unwrap(), 0);
    }

    #[test]
    fn mp_generated_ui_matches_rust_and_invalid_observations_never_borrow() {
        assert_eq!(
            fs::read_to_string(repo_root().join("poc/JAC_PoC_Absorption/UI/JAC_CarrierMP_UI.lua"))
                .unwrap(),
            transport::ui_binding()
        );
        let lua = mp_world(2, false);
        lua.load("MP_PLAN()").exec().unwrap();
        let selection = mp_publish(&lua);
        assert!(mp_receive(&lua, &selection));
        let raw = mp_publish(&lua);
        lua.globals().set("MP_RAW", raw).unwrap();
        for change in [
            "p.Actor=3",
            "p.Turn=999",
            "p.Nonce=999",
            "p.Phase='MOVE'",
            "o.Locations[2]=o.Locations[1]",
            "o.Cities[1].ID=999",
            "o.Locations[1].CreatorName='other'",
        ] {
            let code=format!("local p=JAC_CarrierMP.Decode(MP_RAW);local o=JAC_CarrierMP.Decode(p.Payload);{change};p.Payload=JAC_CarrierMP.Encode(o);return JAC_CarrierMP.Encode(p)");
            let bad = lua.load(code).eval::<String>().unwrap();
            assert!(!mp_receive(&lua, &bad), "{change}");
            assert_eq!(lua.load("return TRANSFERS").eval::<i64>().unwrap(), 0);
        }
    }

    #[test]
    fn resumed_native_phases_refuse_mp_before_any_local_read_or_city_mutation() {
        for phase in ["BORROW", "MOVE", "RETURN", "RESTORE", "PROVED", "RESTORED"] {
            // Reverse the peer order on replay. Each recreation loads the saved
            // intent into an independent Lua 5.1 VM, with different local IDs.
            for local in [2, 3, 3, 2] {
                // Debug is used only to reach the shipped private function;
                // this VM executes trusted repository fixtures, without I/O.
                let lua = world_in(unsafe { Lua::unsafe_new() }, local);
                lua.globals().set("TEST_PHASE", phase).unwrap();
                lua.load(r#"
                  local function find(fn, wanted, seen)
                    if seen[fn] then return end
                    seen[fn]=true
                    for i=1,100 do
                      local name,value=debug.getupvalue(fn,i)
                      if not name then break end
                      if name==wanted then return value end
                      if type(value)=='function' then
                        local found=find(value,wanted,seen)
                        if found then return found end
                      end
                    end
                  end
                  local advance=assert(find(JAC_AbsorptionGameplay.Resume,'AdvanceNativeCarrier',{}))
                  TEST_STATE_EDIT(function(s)
                    local c=s.Transactions.checkpoint.NativeWorks[1].Carrier
                    c.Phase=TEST_PHASE;c.BorrowIssued=true;c.BorrowedCity=c.OriginalCity
                    c.MoveToken=c.Token..':move:1';c.ReturnToken=c.Token..':return'
                  end)
                  local saved=JAC_AbsorptionGameplay.GetState()
                  local before=JAC_AbsorptionRules.Canonical(saved)
                  CityManager.GetCityAt=function() error('local city read reached') end
                  Game.GetLocalPlayer=function() error('local player read reached') end
                  ExposedMembers.JAC_HeroReadAPI={GetObservation=function()
                    error('local observation read reached') end}
                  for replay=1,2 do
                    local ok,err=pcall(advance,saved,saved.Transactions.checkpoint.NativeWorks[1].Carrier)
                    assert(not ok and tostring(err):find('no verified multiplayer actor',1,true),tostring(err))
                    assert(JAC_AbsorptionRules.Canonical(saved)==before)
                    assert(JAC_AbsorptionRules.Canonical(JAC_AbsorptionGameplay.GetState())==before)
                    assert(TRANSFERS==0)
                  end
                "#).exec().unwrap();
            }
        }
    }

    #[test]
    fn saved_route_change_cannot_reuse_a_boundary_packet_or_receipt() {
        let lua = ready();
        let p = message(&lua);
        assert!(receive(&lua, 2, &p));
        lua.load("TEST_STATE_EDIT(function(s) s.Transactions.checkpoint.NativeWorks[1].Carrier.Route='TO_OWNER' end)").exec().unwrap();
        assert!(!receive(&lua, 2, &p));
    }

    #[test]
    fn shipped_binding_matches_rust_and_is_loaded_once_in_gameplay() {
        let script = repo_root().join("poc/JAC_PoC_Absorption/Scripts/JAC_CarrierBoundary.lua");
        assert_eq!(
            fs::read_to_string(script).unwrap(),
            transport::gameplay_binding()
        );
        let text = fs::read_to_string(
            repo_root().join("poc/JAC_PoC_Absorption/JAC_PoC_Absorption.modinfo"),
        )
        .unwrap();
        let doc = roxmltree::Document::parse(&text).unwrap();
        let entries: Vec<_> = doc
            .descendants()
            .filter(|node| {
                node.has_tag_name("AddGameplayScripts")
                    && node.children().any(|file| {
                        file.has_tag_name("File")
                            && file.text() == Some("Scripts/JAC_CarrierBoundary.lua")
                    })
            })
            .collect();
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0].attribute("criteria"),
            Some("JAC_Absorption_Banseiki")
        );
        assert_eq!(
            entries[0]
                .descendants()
                .find(|n| n.has_tag_name("LoadOrder"))
                .unwrap()
                .text(),
            Some("205")
        );
        Lua::new()
            .load(transport::gameplay_binding())
            .into_function()
            .unwrap();
    }

    #[test]
    fn wire_identity_is_length_framed_and_sender_is_selected_in_rust() {
        let lua = ready();
        let p = message(&lua);
        assert_eq!(p["RequestID"], transport::key(&p).unwrap());
        let mut boundary = checkpoint(&lua);
        boundary.local_player = 3;
        assert!(transport::packet(&boundary, &[2, 3]).is_err());
        for ids in [vec![], vec![2, 2], vec![-1, 2]] {
            assert!(transport::packet(&checkpoint(&lua), &ids).is_err());
        }
        assert!(transport::parse_actors("2,no").is_err());
        Lua::new()
            .load(transport::send_query(&p).unwrap())
            .into_function()
            .unwrap();
    }

    #[test]
    fn receiver_rechecks_saved_city_relationship_on_each_gameplay_client() {
        for code in [
            "c.OriginalCity=999",
            "c.Owner=9;c.Borrower=8",
            "c.Phase='MOVE';c.MoveToken=c.BorrowToken;c.Borrower=3;c.BorrowedCity=999",
            "c.Phase='RETURN';c.ReturnToken=c.BorrowToken;c.Borrower=3;c.BorrowedCity=999",
            "c.Phase='PROVED';c.ReturnToken=c.BorrowToken;c.FinalSlot={Owner=3,City=999,X=16,Y=0,Building=1,Slot=0}",
        ] {
            let lua=ready();let mut p=message(&lua);
            lua.load(format!("TEST_STATE_EDIT(function(s) local c=s.Transactions.checkpoint.NativeWorks[1].Carrier;{code} end)")).exec().unwrap();
            let carrier=lua.load("return ExposedMembers.JAC_Absorption.GetState().Transactions.checkpoint.NativeWorks[1].Carrier").eval::<mlua::Table>().unwrap();
            for name in ["OriginalCity","Owner","Borrower","Phase"] {
                let value=carrier.get::<mlua::Value>(name).unwrap();
                p[name]=match value {mlua::Value::String(s)=>json!(s.to_str().unwrap().to_string()),
                    mlua::Value::Integer(n)=>json!(n),mlua::Value::Number(n)=>json!(n as i64),_=>panic!("bad fixture")};
            }
            p["RequestID"]=json!(transport::key(&p).unwrap());
            assert!(!receive(&lua,2,&p),"{code}");
        }
        let lua = ready();
        let mut p = message(&lua);
        lua.load("CityManager.TransferCity(CITIES[16],1)")
            .exec()
            .unwrap();
        p["CityOwner"] = json!(1);
        p["CityID"] = json!(lua.load("return CITIES[16]:GetID()").eval::<i64>().unwrap());
        p["RequestID"] = json!(transport::key(&p).unwrap());
        assert!(!receive(&lua, 2, &p), "unissued BORROW cannot be adopted");
        lua.load("TEST_STATE_EDIT(function(s) s.Transactions.checkpoint.NativeWorks[1].Carrier.BorrowIssued=true end)").exec().unwrap();
        assert!(
            receive(&lua, 2, &p),
            "issued BORROW may precede saved BorrowedCity"
        );
    }

    #[test]
    fn two_gameplay_vms_receive_the_same_packet_without_local_ui_or_a_move() {
        let left = ready();
        let right = ready();
        let p = message(&left);
        for lua in [&left, &right] {
            lua.load("Game.GetLocalPlayer=nil;UI=nil").exec().unwrap();
            let transfers = lua.load("return TRANSFERS").eval::<i64>().unwrap();
            assert!(receive(lua, 2, &p));
            assert!(receive(lua, 2, &p));
            assert!(lua
                .load(transport::receipt_query(&p).unwrap())
                .eval::<bool>()
                .unwrap());
            assert_eq!(
                lua.load("return TRANSFERS").eval::<i64>().unwrap(),
                transfers
            );
        }
        let code = format!(
            "return JAC_AbsorptionRules.Canonical(Game:GetProperty({:?}))",
            transport::STATE_KEY
        );
        assert_eq!(
            left.load(&code).eval::<String>().unwrap(),
            right.load(&code).eval::<String>().unwrap()
        );
    }

    #[test]
    fn stale_sender_turn_phase_city_and_observation_never_write_a_receipt() {
        for name in [
            "Actor",
            "Turn",
            "Phase",
            "CityID",
            "CityOwner",
            "ObservationToken",
            "RequestID",
        ] {
            let lua = ready();
            let mut p = message(&lua);
            p[name] = match name {
                "Phase" => json!("MOVE"),
                "ObservationToken" | "RequestID" => json!("changed"),
                _ => json!(999),
            };
            if name != "RequestID" {
                p["RequestID"] = json!(transport::key(&p).unwrap());
            }
            assert!(!receive(&lua, 2, &p), "{name}");
            assert!(lua
                .load(format!(
                    "return Game:GetProperty({:?})==nil",
                    transport::STATE_KEY
                ))
                .eval::<bool>()
                .unwrap());
        }
        let lua = ready();
        let p = message(&lua);
        assert!(!receive(&lua, 1, &p));
        lua.load("Players[1].IsHuman=function() return true end")
            .exec()
            .unwrap();
        assert!(
            !receive(&lua, 2, &p),
            "Core must independently choose lowest active human"
        );
    }

    #[test]
    fn single_player_rejects_an_otherwise_valid_boundary_without_mutation() {
        let lua = ready();
        let p = message(&lua);
        let before = checkpoint(&lua).ledger;
        lua.load("GameConfiguration.IsAnyMultiplayer=function() return false end")
            .exec()
            .unwrap();
        assert!(!receive(&lua, 2, &p));
        assert!(lua
            .load(format!(
                "return Game:GetProperty({:?})==nil",
                transport::STATE_KEY
            ))
            .eval::<bool>()
            .unwrap());
        assert_eq!(checkpoint(&lua).ledger, before);
    }

    #[test]
    fn saved_receipt_survives_vm_recreation_and_corrupt_payload_is_not_adopted() {
        let first = ready();
        let p = message(&first);
        assert!(receive(&first, 2, &p));
        let captured = checkpoint(&first).transport_receipts.unwrap();
        drop(first);
        let resumed = ready();
        resumed
            .globals()
            .set("TEST_SAVED_BOX", lua_value(&resumed, &captured))
            .unwrap();
        resumed
            .load(format!(
                "Game:SetProperty({:?},TEST_SAVED_BOX)",
                transport::STATE_KEY
            ))
            .exec()
            .unwrap();
        assert!(receive(&resumed, 2, &p));
        assert!(resumed
            .load(transport::receipt_query(&p).unwrap())
            .eval::<bool>()
            .unwrap());
        let id = p["RequestID"].as_str().unwrap();
        resumed.load(format!("local box=Game:GetProperty({:?});box.Receipts[{:?}].Packet.CityID=999;Game:SetProperty({:?},box)",transport::STATE_KEY,id,transport::STATE_KEY)).exec().unwrap();
        assert!(!resumed
            .load(transport::receipt_query(&p).unwrap())
            .eval::<bool>()
            .unwrap());
        assert!(!receive(&resumed, 2, &p));
    }

    #[test]
    fn runner_confirms_saved_receipt_and_unknown_send_is_never_repeated() {
        let lua = ready();
        let raw = serde_json::to_string(&checkpoint(&lua)).unwrap();
        let p = message(&lua);
        let fake = FakeBackend::new().lua(move |_, code| {
            if code.contains("local wanted") {
                Ok(raw.clone())
            } else if code == transport::ACTORS_QUERY {
                Ok("2".into())
            } else if code.contains("UI.RequestPlayerOperation") {
                Err(Error::unknown("lost send response"))
            } else {
                Ok("false".into())
            }
        });
        let mut h = Harness::new(json!({}), fake);
        let step = json!({"action":"carrier_exchange","token":TOKEN,"candidate":CANDIDATE,"direct_apply":true});
        scenario::validate_scenario(&json!({"name":"boundary","steps":[step.clone()]})).unwrap();
        assert!(h.runner.run_step(&step).is_err());
        assert_eq!(
            h.calls()
                .iter()
                .filter(|(_, c)| c.contains("UI.RequestPlayerOperation"))
                .count(),
            1
        );
        let raw = serde_json::to_string(&checkpoint(&ready())).unwrap();
        let expected_query = transport::receipt_query(&p).unwrap();
        let fake = FakeBackend::new().lua(move |_, code| {
            if code.contains("local wanted") {
                Ok(raw.clone())
            } else if code == transport::ACTORS_QUERY {
                Ok("2".into())
            } else {
                assert_eq!(code, expected_query);
                Ok("true".into())
            }
        });
        let mut h = Harness::new(json!({}), fake);
        h.runner.run_step(&step).unwrap();
        assert!(!h
            .calls()
            .iter()
            .any(|(_, c)| c.contains("UI.RequestPlayerOperation")));
    }
}
