//! Native-carrier witness controller; separate from the growth witness.
use civ6_autotest_rs::{Error, Result};
use clap::{Parser, Subcommand};
use std::{fs, path::PathBuf};

#[path = "../hotseat_native_carrier.rs"]
mod protocol;

#[derive(Parser)]
#[command(about = "Validate the fixed native-carrier hotseat log protocol")]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Audit {
        input: PathBuf,
        #[arg(long)]
        nonce: String,
    },
}

fn run(args: Args) -> Result<()> {
    match args.command {
        Command::Audit { input, nonce } => {
            let frames = protocol::parse_available_frames(&fs::read_to_string(input)?, &nonce)?;
            let mut receipts = protocol::Receipts::default();
            for frame in &frames {
                receipts.accept(frame)?;
            }
            println!("{}", serde_json::to_string_pretty(&frames)?);
        }
    }
    Ok(())
}

fn main() {
    if let Err(error) = run(Args::parse()) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
