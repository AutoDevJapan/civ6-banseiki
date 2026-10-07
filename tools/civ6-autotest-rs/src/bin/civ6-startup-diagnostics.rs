//! Internal, bounded worker; it can only inspect one explicitly pinned game.
#[cfg(all(windows, not(test)))]
#[path = "../../../civ6-native-bridge-rs/src/bin/civ6-native-bridge/bridge_controller.rs"]
mod bootstrap_native_controller;
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    #[cfg(all(windows, not(test)))]
    if args.first().is_some_and(|s| s == "bootstrap") {
        if let Err(e) = bootstrap_native_controller::run() {
            eprintln!("{e}");
            std::process::exit(1);
        }
        return;
    }
    let result = civ6_autotest_rs::native_order::worker(&args)
        .unwrap_or_else(|| civ6_autotest_rs::startup::diagnostic_worker(&args));
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
