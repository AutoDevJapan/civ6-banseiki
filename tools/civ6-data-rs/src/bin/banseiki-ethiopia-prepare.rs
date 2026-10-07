//! Local DLC preparation; no official data is added to the checked-in MOD.
use anyhow::{bail, Context, Result};
use civ6_data_rs::{ethiopia, Repository};
use std::path::PathBuf;

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let mut game = None;
    let mut output = None;
    let mut refresh = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--game-dir" => game = Some(PathBuf::from(args.next().context("--game-dir value")?)),
            "--output" => output = Some(PathBuf::from(args.next().context("--output value")?)),
            "--refresh" => refresh = true,
            _ => bail!("usage: banseiki-ethiopia-prepare --game-dir <installed game> --output <absolute MOD folder> [--refresh]"),
        }
    }
    let output = output.context("--output required")?;
    ethiopia::prepare_local_dlc_mod(
        &Repository::checked_in(),
        &game.context("--game-dir required")?,
        &output,
        refresh,
    )?;
    println!("prepared {}", output.display());
    Ok(())
}
