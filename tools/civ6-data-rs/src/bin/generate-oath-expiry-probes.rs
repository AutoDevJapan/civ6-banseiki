//! Natural reign-change expiry probe from a verified save run (civ6_data_rs::oath_expiry_probe).
use civ6_data_rs::{oath_expiry_probe, Repository};

fn main() {
    if let Err(error) = oath_expiry_probe::run(
        &Repository::checked_in(),
        &std::env::args().skip(1).collect::<Vec<_>>(),
    ) {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}
