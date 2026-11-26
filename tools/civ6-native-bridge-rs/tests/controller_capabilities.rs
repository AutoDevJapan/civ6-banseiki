#[cfg(windows)]
#[test]
fn controller_negotiates_offline_without_pid_or_lease() {
    use std::os::windows::process::CommandExt;
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_civ6-native-bridge"))
        .arg("--capabilities")
        .env_remove("CIV6_LIVE_CHAIN_PID")
        .env_remove("CIV6_AUTOTEST_CONTROLLER_PID")
        .creation_flags(0x08000000)
        .output()
        .unwrap();
    assert!(output.status.success(), "{:?}", output);
    assert!(output.stderr.is_empty());
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap(),
        civ6_native_bridge_rs::controller_capabilities()
    );
}
