//! Local DLC preparation; no official data is added to the checked-in MOD.
use anyhow::{bail, Context, Result};
use civ6_data_rs::{ethiopia, Repository};
use std::path::PathBuf;

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let mut game = None;
    let mut output = None;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--game-dir" => game = Some(PathBuf::from(args.next().context("--game-dir value")?)),
            "--output" => output = Some(PathBuf::from(args.next().context("--output value")?)),
            _ => bail!("usage: banseiki-ethiopia-prepare --game-dir <installed game> --output <new absolute MOD folder>"),
        }
    }
    let output = output.context("--output required")?;
    ethiopia::prepare(
        &Repository::checked_in(),
        &game.context("--game-dir required")?,
        &output,
    )?;
    println!("prepared {}", output.display());
    Ok(())
}
