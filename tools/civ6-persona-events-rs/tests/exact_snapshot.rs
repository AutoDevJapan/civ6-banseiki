use civ6_persona_events_rs::sp_runtime::SERIAL;
use mlua::{Function, Lua};

fn serializer(lua: &Lua) -> Function {
    lua.load(format!("{SERIAL}\nreturn serial")).eval().unwrap()
}

#[test]
fn nearby_payment_sequence_and_spending_numbers_cannot_compare_equal() {
    let lua = Lua::new();
    let serial = serializer(&lua);
    // Both sequences satisfy the normal acquisition store's integer bound.
    // Lua 5.1 tostring rounds them to the same 14-digit representation.
    for (before, after) in [
        (9_007_199_254_740_989.0, 9_007_199_254_740_990.0),
        (100_000_000_000_001.0, 100_000_000_000_002.0),
        (1.0, 1.000_000_000_000_000_2),
    ] {
        assert_ne!(before, after);
        let previous = lua.create_table().unwrap();
        previous.set("sequence", before).unwrap();
        let current = lua.create_table().unwrap();
        current.set("sequence", after).unwrap();
        assert_ne!(
            serial.call::<String>(previous).unwrap(),
            serial.call::<String>(current).unwrap(),
            "a changed receipt number must fail full save/reload comparison"
        );
    }
}

#[test]
fn scalar_numbers_round_trip_through_independent_lua51_reads() {
    let lua = Lua::new();
    let serial = serializer(&lua);
    for value in [
        0.0,
        -0.0,
        0.1,
        1.000_000_000_000_000_2,
        9_007_199_254_740_990.0,
        f64::from_bits(1),
        -f64::from_bits(1),
        f64::MIN_POSITIVE,
        f64::MAX,
    ] {
        let encoded: String = serial.call(value).unwrap();
        let tail = encoded.strip_prefix("number:").unwrap();
        let (size, number) = tail.split_once(':').unwrap();
        assert_eq!(size.parse::<usize>().unwrap(), number.len());
        let restored: f64 = lua
            .load(format!("return tonumber('{number}')"))
            .eval()
            .unwrap();
        assert_eq!(restored, value);
    }
    assert_eq!(
        serial.call::<String>(0.0).unwrap(),
        serial.call::<String>(-0.0).unwrap()
    );
}

#[test]
fn typed_keys_have_one_order_across_fresh_contexts_and_insertion_orders() {
    let mut captured = Vec::new();
    for code in [
        "local x={}; x[1]='number'; x['1']='string'; x[true]='boolean'; return x",
        "local x={}; x[true]='boolean'; x['1']='string'; x[1]='number'; return x",
        "return {['1']='string', [true]='boolean', [1]='number'}",
    ] {
        let lua = Lua::new();
        let table = lua.load(code).eval::<mlua::Table>().unwrap();
        captured.push(serializer(&lua).call::<String>(table).unwrap());
    }
    assert_eq!(captured[0], captured[1]);
    assert_eq!(captured[0], captured[2]);
}

#[test]
fn strings_boolean_and_numeric_types_and_all_nested_fields_are_retained() {
    let lua = Lua::new();
    let serial = serializer(&lua);
    let variants = [
        "return {owner=0, person='伊達', acquired={[1]={sequence=100000000000001, spent=1}}, flag=true}",
        "return {owner=0, person='伊達', acquired={[1]={sequence=100000000000002, spent=1}}, flag=true}",
        "return {owner=0, person='伊達', acquired={[1]={sequence=100000000000001, spent=1}}, flag='true'}",
        "return {owner=0, person='伊達', acquired={['1']={sequence=100000000000001, spent=1}}, flag=true}",
    ];
    let mut records = std::collections::BTreeSet::new();
    for variant in variants {
        let table = lua.load(variant).eval::<mlua::Table>().unwrap();
        assert!(records.insert(serial.call::<String>(table).unwrap()));
    }
    let text = lua.create_string("a\0b\n{;=:}").unwrap();
    assert_eq!(serial.call::<String>(text).unwrap(), "string:9:a\0b\n{;=:}");
}

#[test]
fn invalid_values_cannot_become_pointer_or_nonfinite_payment_records() {
    let lua = Lua::new();
    let serial = serializer(&lua);
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(serial.call::<String>(invalid).is_err());
    }
    for code in [
        "return {value=function() end}",
        "return {value=coroutine.create(function() end)}",
        "local t={}; t[t]='unstable key'; return t",
        "local t={}; t.self=t; return t",
    ] {
        assert!(
            serial
                .call::<String>(lua.load(code).eval::<mlua::Value>().unwrap())
                .is_err(),
            "{code}"
        );
    }
    struct Unsupported;
    impl mlua::UserData for Unsupported {}
    let opaque = lua.create_userdata(Unsupported).unwrap();
    assert!(serial.call::<String>(opaque).is_err());
}

#[test]
fn repeated_tables_and_deep_copies_keep_the_same_value() {
    let first = Lua::new();
    let a = first
        .load("local x={owner=0, sequence=9007199254740990}; return {base=x, copy=x}")
        .eval::<mlua::Table>()
        .unwrap();
    let before = serializer(&first).call::<String>(a).unwrap();
    let second = Lua::new();
    let b = second.load("return {copy={sequence=9007199254740990,owner=0}, base={owner=0,sequence=9007199254740990}}").eval::<mlua::Table>().unwrap();
    assert_eq!(before, serializer(&second).call::<String>(b).unwrap());
}
