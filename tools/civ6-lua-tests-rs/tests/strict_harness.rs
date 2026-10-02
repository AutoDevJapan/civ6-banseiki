use civ6_lua_tests_rs::new_lua;
use civ6_lua_tests_rs::strict::{
    assert_boolean_identity, assert_python_eq, assert_python_truth, ChunkExt, Data, Key, TableExt,
};
use mlua::{Table, Value};
use serde_json::json;
use std::collections::BTreeMap;

#[test]
fn numeric_equality_accepts_int_float_but_rejects_strings_booleans_nil_and_fractions() {
    let lua = new_lua();
    for code in ["return 120", "return 120.0"] {
        assert_eq!(lua.load(code).strict_eval::<i64>().unwrap(), 120);
        assert_eq!(lua.load(code).strict_eval::<Data>().unwrap(), json!(120));
    }
    for code in ["return '120'", "return true", "return nil", "return 120.5"] {
        assert!(lua.load(code).strict_eval::<i64>().is_err(), "{code}");
        assert_ne!(
            lua.load(code).strict_eval::<Data>().unwrap(),
            json!(120),
            "{code}"
        );
    }
    assert!(lua.load("return '1.5'").strict_eval::<f64>().is_err());
    assert_eq!(lua.load("return 1.5").strict_eval::<f64>().unwrap(), 1.5);
}

#[test]
fn boolean_identity_rejects_nil_zero_strings_and_missing_results() {
    let lua = new_lua();
    assert!(!lua.load("return false").strict_eval::<bool>().unwrap());
    assert!(lua.load("return true").strict_eval::<bool>().unwrap());
    for code in ["return nil", "return 0", "return 'false'", "return"] {
        assert!(lua.load(code).strict_eval::<bool>().is_err(), "{code}");
    }
    assert_eq!(
        lua.load("return nil")
            .strict_eval::<Option<bool>>()
            .unwrap(),
        None
    );
    assert_eq!(
        lua.load("return false")
            .strict_eval::<Option<bool>>()
            .unwrap(),
        Some(false)
    );
}

#[test]
fn full_returns_reject_extra_and_missing_results_before_decoding() {
    let lua = new_lua();
    assert_eq!(
        lua.load("return false,'PRODUCING'")
            .strict_eval::<(bool, String)>()
            .unwrap(),
        (false, "PRODUCING".into())
    );
    for code in [
        "return false,'PRODUCING',99",
        "return false,'PRODUCING',nil",
        "return false",
        "return nil,'PRODUCING'",
        "return 'false','PRODUCING'",
        "return false,99",
    ] {
        assert!(
            lua.load(code).strict_eval::<(bool, String)>().is_err(),
            "{code}"
        );
    }
    assert_eq!(
        lua.load("return 100.0,5")
            .strict_eval::<(i64, i64)>()
            .unwrap(),
        (100, 5)
    );
    for code in ["return 100,5,99", "return '100',5", "return 100,5.5"] {
        assert!(
            lua.load(code).strict_eval::<(i64, i64)>().is_err(),
            "{code}"
        );
    }
    assert!(lua.load("return 120,99").strict_eval::<i64>().is_err());
    let raw_results = lua
        .load("return false,'PRODUCING',99")
        .eval::<mlua::MultiValue>()
        .unwrap();
    assert_eq!(raw_results.len(), 3);
    assert!(lua
        .load("return false,...,99")
        .strict_call::<(bool, String)>("PRODUCING")
        .is_err());
}

#[test]
fn table_fields_and_sequences_do_not_coerce_values_or_hide_extra_keys() {
    let lua = new_lua();
    let table = lua
        .load("return {number='120', text=120, flag=nil, [1]='ok'}")
        .strict_eval::<Table>()
        .unwrap();
    assert!(table.strict_get::<i64>("number").is_err());
    assert!(table.strict_get::<String>("text").is_err());
    assert!(table.strict_get::<bool>("flag").is_err());
    for code in [
        "return {'ok',99}",
        "return {[1]='ok',[3]='hidden'}",
        "return {[1]='ok',['1']='collision'}",
        "return {[1]='ok',extra='hidden'}",
    ] {
        let table = lua.load(code).strict_eval::<Table>().unwrap();
        assert!(
            table
                .strict_sequence_values::<String>()
                .collect::<mlua::Result<Vec<_>>>()
                .is_err(),
            "{code}"
        );
    }
    let table = lua
        .load("return {'ok','second'}")
        .strict_eval::<Table>()
        .unwrap();
    assert_eq!(
        table
            .strict_sequence_values::<String>()
            .collect::<mlua::Result<Vec<_>>>()
            .unwrap(),
        ["ok", "second"]
    );
}

#[test]
fn dictionaries_preserve_key_types_and_collisions_at_every_depth() {
    let lua = new_lua();
    let numeric = lua
        .load("return {nested={[0]='same',[2]='same',[3]='same'}}")
        .strict_eval::<Data>()
        .unwrap();
    let strings = lua
        .load("return {nested={['0']='same',['2']='same',['3']='same'}}")
        .strict_eval::<Data>()
        .unwrap();
    let collision = lua
        .load("return {nested={[0]='same',['0']='same',[2]='same',[3]='same'}}")
        .strict_eval::<Data>()
        .unwrap();
    assert_ne!(numeric, strings);
    assert_ne!(numeric, collision);
    assert_eq!(
        numeric["nested"]
            .entries()
            .keys()
            .cloned()
            .collect::<Vec<_>>(),
        [Key::Integer(0), Key::Integer(2), Key::Integer(3)]
    );
    assert_eq!(collision["nested"].entries().len(), 4);
    assert_eq!(strings["nested"], json!({"0":"same","2":"same","3":"same"}));
    assert_ne!(numeric["nested"], json!({"0":"same","2":"same","3":"same"}));
}

#[test]
fn nil_missing_empty_table_and_multivalue_tuple_are_distinct() {
    let lua = new_lua();
    let empty = lua.load("return {}").strict_eval::<Data>().unwrap();
    let nil = lua.load("return nil").strict_eval::<Data>().unwrap();
    assert_ne!(empty, nil);
    assert_ne!(empty["missing"], nil);
    assert_ne!(empty["missing"], empty);
    assert!(empty.get("missing").is_none());
    assert!(std::panic::catch_unwind(|| nil.get("missing")).is_err());
    assert_eq!(empty, json!({}));
    assert_eq!(nil, json!(null));
    let results = Data::from_lupa_results(
        lua.load("return false,'PRODUCING',99")
            .eval::<mlua::MultiValue>()
            .unwrap(),
    )
    .unwrap();
    assert_ne!(
        results,
        Data::Results(vec![json!(false).into(), json!("PRODUCING").into()])
    );
    assert_ne!(results, json!([false, "PRODUCING", 99]));
    assert_eq!(results[0], false);
}

#[test]
fn snapshot_restore_preserves_mixed_keys_and_numeric_equality_in_a_new_vm() {
    let lua = new_lua();
    let before = lua.load("return {nested={[0]='number',['0']='string',[1.5]='fraction',[true]='boolean'}, empty={}}").strict_eval::<Data>().unwrap();
    let other = new_lua();
    other.globals().set("saved", before.to_lua(&other)).unwrap();
    let after = other.load("return saved").strict_eval::<Data>().unwrap();
    assert_eq!(before, after);
    other.load("saved.nested[0]=nil").exec().unwrap();
    assert_ne!(
        other.load("return saved").strict_eval::<Data>().unwrap(),
        before
    );
    let ordinary = lua
        .load("return {score=120.0,parts={1,2}}")
        .strict_eval::<Data>()
        .unwrap();
    assert_eq!(ordinary, json!({"score":120,"parts":[1,2]}));
    assert_eq!(
        ordinary.entries().keys().cloned().collect::<Vec<_>>(),
        [Key::String("parts".into()), Key::String("score".into())]
    );
}

#[test]
fn python_truth_checks_remain_separate_from_boolean_identity() {
    let lua = new_lua();
    for code in [
        "return nil",
        "return false",
        "return 0",
        "return ''",
        "return",
    ] {
        assert!(!lua.load(code).python_truthy().unwrap(), "{code}");
    }
    for code in [
        "return true",
        "return 1",
        "return '120'",
        "return {}",
        "return false,'PRODUCING'",
    ] {
        assert!(lua.load(code).python_truthy().unwrap(), "{code}");
    }
    let value = Value::Number(0.0);
    assert!(!civ6_lua_tests_rs::strict::truthy(&value));
    assert!(!Data::Object(BTreeMap::new()).python_truthy());
}

#[test]
fn snapshot_rejects_cycles_but_accepts_shared_tables() {
    let lua = new_lua();
    assert!(lua
        .load("local t={};t.self=t;return t")
        .strict_eval::<Data>()
        .is_err());
    assert_eq!(
        lua.load("local t={score=1};return {a=t,b=t}")
            .strict_eval::<Data>()
            .unwrap(),
        json!({"a":{"score":1},"b":{"score":1}})
    );
}

#[test]
fn python_equality_recurses_through_numeric_booleans_without_coercing_text_or_nil() {
    let lua = new_lua();
    for code in ["return true", "return 1", "return 1.0"] {
        assert_python_eq(&lua.load(code).python_eval().unwrap(), true);
        assert_python_eq(&lua.load(code).python_eval().unwrap(), 1_i64);
    }
    for code in ["return false", "return 0", "return -0.0"] {
        assert_python_eq(&lua.load(code).python_eval().unwrap(), false);
        assert_python_eq(&lua.load(code).python_eval().unwrap(), 0_i64);
    }
    assert_python_eq(
        &lua.load("return {boosted={['2']=1,['4']=true}, parts={false,1.0}}")
            .python_eval()
            .unwrap(),
        json!({"boosted":{"2":true,"4":1},"parts":[0,true]}),
    );
    assert_python_eq(
        &lua.load("return 0,'PRODUCING'").python_eval().unwrap(),
        (false, "PRODUCING"),
    );
    assert_python_eq(
        &lua.load("return true,false,{flag=1}")
            .python_eval()
            .unwrap(),
        (1_i64, 0_i64, json!({"flag":true})),
    );
    for code in [
        "return '0'",
        "return 'false'",
        "return nil",
        "return",
        "return 0.5",
        "return {}",
        "return 0,nil",
    ] {
        assert!(
            !lua.load(code)
                .python_eval()
                .unwrap()
                .python_eq(&Data::Boolean(false)),
            "{code}"
        );
    }
    assert_ne!(
        lua.load("return {flag='1'}").python_eval().unwrap(),
        json!({"flag":true})
    );
    assert_ne!(
        lua.load("return {flag=1,extra=99}").python_eval().unwrap(),
        json!({"flag":true})
    );
    assert_ne!(
        lua.load("return {0,'PRODUCING'}").python_eval().unwrap(),
        Data::from((false, "PRODUCING"))
    );
}

#[test]
fn plain_container_truth_and_raw_lupa_table_truth_are_distinct_from_identity() {
    let lua = new_lua();
    for code in [
        "return false",
        "return nil",
        "return 0",
        "return ''",
        "return {}",
    ] {
        assert_python_truth(&lua.load(code).python_eval().unwrap(), false);
    }
    for code in [
        "return 1",
        "return 'false'",
        "return {0}",
        "return {flag=false}",
        "return false,nil",
    ] {
        assert_python_truth(&lua.load(code).python_eval().unwrap(), true);
    }
    assert!(lua.load("return {}").python_truthy().unwrap());
    assert_boolean_identity(&lua.load("return false").python_eval().unwrap(), false);
    for code in [
        "return 0",
        "return nil",
        "return 'false'",
        "return {}",
        "return false,nil",
        "return",
    ] {
        let value = lua.load(code).python_eval().unwrap();
        assert!(
            std::panic::catch_unwind(|| assert_boolean_identity(&value, false)).is_err(),
            "{code}"
        );
    }
}

#[test]
fn values_enumerate_complete_sparse_and_string_keyed_rows_and_empty_checks_use_pairs() {
    let lua = new_lua();
    let empty = lua.load("return {}").strict_eval::<Table>().unwrap();
    assert!(empty.all_entries_empty().unwrap());
    for code in [
        "return {unexpected=99}",
        "return {[3]=99}",
        "return {[false]=99}",
    ] {
        let table = lua.load(code).strict_eval::<Table>().unwrap();
        assert!(!table.all_entries_empty().unwrap(), "{code}");
        assert_eq!(
            table
                .strict_values::<Data>()
                .collect::<mlua::Result<Vec<_>>>()
                .unwrap(),
            [Data::Integer(99)]
        );
    }
    let rows = lua
        .load("return {[1]={1,2},[3]={3,4},extra={5,6}}")
        .strict_eval::<Table>()
        .unwrap()
        .strict_values::<Table>()
        .collect::<mlua::Result<Vec<_>>>()
        .unwrap();
    assert_eq!(rows.len(), 3);
    for code in ["return {1,2,99}", "return {[1]=1,[2]=2,extra=99}"] {
        let row = lua.load(code).strict_eval::<Table>().unwrap();
        let actual = Data::Results(row.strict_values().collect::<mlua::Result<_>>().unwrap());
        assert_ne!(actual, Data::from((1_i64, 2_i64)), "{code}");
    }
}

#[test]
fn successful_raw_calls_can_fail_decoding_without_becoming_lua_execution_errors() {
    let lua = new_lua();
    for code in ["return function() end", "local t={};t.self=t;return t"] {
        let raw = lua
            .load(code)
            .call::<mlua::MultiValue>(())
            .expect("successful raw Lua call");
        assert!(Data::from_lupa_results(raw).is_err(), "{code}");
    }
    assert!(lua
        .load("error('intentional refusal')")
        .call::<mlua::MultiValue>(())
        .is_err());
}
