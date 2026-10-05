use civ6_autotest_rs::{
    carrier_sync::{compare, query_with_scope, Checkpoint},
    Result,
};
use clap::{Parser, Subcommand};
use std::{fs, path::PathBuf};

#[derive(Parser)]
#[command(about = "Collect or compare native carrier evidence; does not enable multiplayer")]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Query {
        token: String,
        candidate: String,
        #[arg(long)]
        direct_apply: bool,
    },
    Compare {
        before: PathBuf,
        after: PathBuf,
        #[arg(long)]
        peers: bool,
    },
}

fn run(args: Args) -> Result<()> {
    match args.command {
        Command::Query {
            token,
            candidate,
            direct_apply,
        } => print!("{}", query_with_scope(&token, &candidate, !direct_apply)?),
        Command::Compare {
            before,
            after,
            peers,
        } => {
            let left: Checkpoint = serde_json::from_slice(&fs::read(before)?)?;
            let right: Checkpoint = serde_json::from_slice(&fs::read(after)?)?;
            compare(&left, &right, peers)?;
            println!("MATCH: {} boundary", if peers { "peer" } else { "reload" });
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
