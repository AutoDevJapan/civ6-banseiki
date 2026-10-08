use civ6_autotest_rs::{
    growth_sync::{compare, Checkpoint, COLLECTION_QUERY},
    tuner::TunerClient,
    Error, Result,
};
use clap::{Parser, Subcommand};
use std::{fs, path::PathBuf};

#[derive(Parser)]
#[command(
    about = "Read LAN growth records or compare supplied captures; does not establish a network barrier"
)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Print the fixed read-only InGame query, without connecting to a game.
    Query,
    /// Run once against this machine's loopback FireTuner. Use only in an owned live_chain session.
    Capture {
        #[arg(long)]
        endpoint: String,
        #[arg(long)]
        session: String,
        #[arg(long)]
        barrier: String,
        #[arg(long)]
        phase: String,
        #[arg(long, default_value_t = 4318)]
        port: u16,
    },
    /// Validate and compare two independently collected checkpoint files.
    Compare { left: PathBuf, right: PathBuf },
}

fn run(args: Args) -> Result<()> {
    match args.command {
        Command::Query => print!("{COLLECTION_QUERY}"),
        Command::Capture {
            endpoint,
            session,
            barrier,
            phase,
            port,
        } => {
            // Validate caller labels before any socket or Lua operation.
            for (name, label) in [
                ("endpoint", &endpoint),
                ("session", &session),
                ("barrier", &barrier),
                ("phase", &phase),
            ] {
                if label.trim().is_empty() || label.chars().any(char::is_control) {
                    return Err(Error::input(format!("invalid {name} label")));
                }
            }
            let mut client = TunerClient::connect(port, 5.0)?;
            let result = client.run_lua("InGame", COLLECTION_QUERY, 5.0);
            client.close();
            let checkpoint = Checkpoint {
                endpoint_id: endpoint,
                session_tag: session,
                barrier_tag: barrier,
                phase,
                observation: result?.value,
            };
            checkpoint.parse()?;
            println!("{}", serde_json::to_string_pretty(&checkpoint)?);
        }
        Command::Compare { left, right } => {
            let left: Checkpoint = serde_json::from_slice(&fs::read(left)?)?;
            let right: Checkpoint = serde_json::from_slice(&fs::read(right)?)?;
            compare(&left, &right)?;
            println!("MATCH: supplied growth records; LAN timing, save/load and rejoin remain separately verifiable");
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
