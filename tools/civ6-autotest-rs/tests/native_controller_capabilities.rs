use civ6_autotest_rs::native_bridge::verify_controller_capabilities;
use civ6_native_bridge_rs::controller_capabilities;

#[test]
fn old_shared_controller_is_rejected_before_game_operations() {
    let mut old = controller_capabilities();
    old["modes"]
        .as_array_mut()
        .unwrap()
        .retain(|mode| mode != "paid-assignment-runtime");
    assert!(verify_controller_capabilities(true, &serde_json::to_vec(&old).unwrap()).is_err());
    for raw in [
        b"".as_slice(),
        b"bridge failed: invalid bridge mode",
        b"null",
    ] {
        assert!(verify_controller_capabilities(false, raw).is_err());
        assert!(verify_controller_capabilities(true, raw).is_err());
    }
}

#[test]
fn current_protocol_requires_success_and_matching_runtime_and_core() {
    let current = controller_capabilities();
    let bytes = serde_json::to_vec(&current).unwrap();
    verify_controller_capabilities(true, &bytes).unwrap();
    assert!(verify_controller_capabilities(false, &bytes).is_err());
    for key in ["schema", "runtime_config_version", "gamecore_sha256"] {
        let mut bad = current.clone();
        bad[key] = serde_json::Value::Null;
        assert!(verify_controller_capabilities(true, &serde_json::to_vec(&bad).unwrap()).is_err());
    }
}
