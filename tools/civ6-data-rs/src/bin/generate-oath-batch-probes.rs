//! Measure ten or more yield-only oaths in one launch and one native save (civ6_data_rs::oath_batch_probe).
use civ6_data_rs::{oath_batch_probe, Repository};

fn main() {
    if let Err(error) = oath_batch_probe::run(
        &Repository::checked_in(),
        &std::env::args().skip(1).collect::<Vec<_>>(),
    ) {
        eprintln!("{error:#}");
        std::process::exit(1);
    }
}
