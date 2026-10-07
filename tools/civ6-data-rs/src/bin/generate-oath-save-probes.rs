//! Extend a numeric oath scenario with a real save/reload (civ6_data_rs::oath_save_probe).
use civ6_data_rs::{oath_save_probe, Repository};

fn main() {
    if let Err(error) = oath_save_probe::run(
        &Repository::checked_in(),
        &std::env::args().skip(1).collect::<Vec<_>>(),
    ) {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}
