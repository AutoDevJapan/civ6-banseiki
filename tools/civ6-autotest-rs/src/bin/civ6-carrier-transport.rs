use civ6_autotest_rs::{carrier_sync::Checkpoint, carrier_transport as transport, Result};
use clap::{Parser, Subcommand};
use std::{fs, path::PathBuf};

#[derive(Parser)]
#[command(
    about = "Generate the native carrier PlayerOperation binding and scalar boundary packets"
)]
struct Args {
    #[command(subcommand)]
    command: Command,
}
#[derive(Subcommand)]
enum Command {
    Generate {
        output: PathBuf,
    },
    GenerateUi {
        output: PathBuf,
    },
    SendQuery {
        checkpoint: PathBuf,
        active_humans: String,
    },
}
fn run(args: Args) -> Result<()> {
    match args.command {
        Command::Generate { output } => fs::write(output, transport::gameplay_binding())?,
        Command::GenerateUi { output } => fs::write(output, transport::ui_binding())?,
        Command::SendQuery {
            checkpoint,
            active_humans,
        } => {
            let checkpoint: Checkpoint = serde_json::from_slice(&fs::read(checkpoint)?)?;
            let actors = transport::parse_actors(&active_humans)?;
            print!(
                "{}",
                transport::send_query(&transport::packet(&checkpoint, &actors)?)?
            );
        }
    }
    Ok(())
}
fn main() {
    if let Err(e) = run(Args::parse()) {
        eprintln!("{e}");
        std::process::exit(1);
    }
}
