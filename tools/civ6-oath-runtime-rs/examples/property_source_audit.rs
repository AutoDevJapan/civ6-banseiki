//! Read-only source audit; this never loads a DLL or calls a game function.
use civ6_oath_runtime_rs::preflight::{PropertySourcePlan, CORE_PATH};

fn main() {
    let bytes = std::fs::read(CORE_PATH).expect("read pinned Core source");
    PropertySourcePlan::from_disk(&bytes).expect("native property source identity");
    println!("property_source_matched=true native_called=false quantity_accepted=false");
}
