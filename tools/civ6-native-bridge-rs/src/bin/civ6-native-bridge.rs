//! Bounded controller for the separate telemetry-only GameCore bridge.
//! It loads the DLL only into the live_chain-owned, pinned DX11 game. The DLL
//! tail-jumps to the original CanEver; this controller never grants admission.
#[cfg(not(windows))]
fn main() {
    eprintln!("civ6-native-bridge requires Windows x64");
    std::process::exit(2);
}

#[cfg(windows)]
#[path = "civ6-native-bridge/bridge_controller.rs"]
mod windows;

#[cfg(windows)]
fn main() {
    if std::env::args().skip(1).collect::<Vec<_>>() == ["--capabilities"] {
        println!("{}", civ6_native_bridge_rs::controller_capabilities());
        return;
    }
    if let Err(error) = windows::run() {
        eprintln!("bridge failed: {error}");
        std::process::exit(1);
    }
}
