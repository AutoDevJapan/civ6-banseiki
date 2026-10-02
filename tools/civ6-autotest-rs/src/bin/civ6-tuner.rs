use civ6_autotest_rs::tuner::{PortArgument, TunerClient};
use clap::{Parser, ValueEnum};

#[derive(Clone, Copy, ValueEnum)]
enum Command {
    States,
    Lua,
}

#[derive(Parser)]
#[command(about = "Inspect a running local Civ VI FireTuner")]
struct Args {
    command: Command,
    #[arg(long, default_value = "InGame")]
    state: String,
    #[arg(long)]
    code: Option<String>,
    #[arg(long, default_value = "4318", allow_hyphen_values = true)]
    port: PortArgument,
}

fn main() -> std::process::ExitCode {
    let args = Args::parse();
    if matches!(args.command, Command::Lua) && args.code.as_deref().is_none_or(str::is_empty) {
        eprintln!("error: lua requires --code");
        return std::process::ExitCode::from(2);
    }
    match run(args) {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            std::process::ExitCode::FAILURE
        }
    }
}

fn run(args: Args) -> civ6_autotest_rs::Result<()> {
    let mut client = TunerClient::connect(args.port.socket_port()?, 5.0)?;
    match args.command {
        Command::States => {
            println!("{}", client.identity);
            for (index, name) in client.list_state_pairs(5.0)? {
                println!("{index}: {name}");
            }
        }
        Command::Lua => {
            let result = client.run_lua(
                &args.state,
                args.code.as_deref().expect("validated code"),
                5.0,
            )?;
            println!("{}", result.output);
        }
    }
    client.close();
    Ok(())
}
