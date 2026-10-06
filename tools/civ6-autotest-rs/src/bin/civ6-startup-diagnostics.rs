//! Internal, bounded worker; it can only inspect one explicitly pinned game.
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let result = civ6_autotest_rs::native_order::worker(&args)
        .unwrap_or_else(|| civ6_autotest_rs::startup::diagnostic_worker(&args));
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
