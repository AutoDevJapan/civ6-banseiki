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
    let lua = Lua::new();
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
        "#).exec().unwrap();
        lua.load(transport::gameplay_binding()).exec().unwrap();
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
