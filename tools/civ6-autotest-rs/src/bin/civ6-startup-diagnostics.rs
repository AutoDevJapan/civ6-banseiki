//! Internal, bounded worker; it can only inspect one explicitly pinned game.
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Err(error) = civ6_autotest_rs::startup::diagnostic_worker(&args) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
