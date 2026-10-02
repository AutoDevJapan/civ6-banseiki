use civ6_autotest_rs::tuner::{PortArgument, TunerClient};
use clap::{Parser, ValueEnum};

#[derive(Clone, Copy, ValueEnum)]
enum Command {
    States,
    Lua,
}

#[derive(Parser)]
#[command(
    about = "Inspect a running local Civ VI FireTuner",
    infer_long_args = true,
    args_override_self = true,
    allow_negative_numbers = true
)]
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

#[cfg(test)]
#[path = "support/parser_tests.rs"]
mod parser_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compat_tuner_negative_number_strings_preserve_values_and_last_occurrences() {
        let args = Args::try_parse_from([
            "civ6-tuner",
            "lua",
            "--state",
            "-1",
            "--s",
            "InGame",
            "--code",
            "-2.5",
        ])
        .unwrap();
        assert_eq!(args.state, "InGame");
        assert_eq!(args.code.as_deref(), Some("-2.5"));
        let args = Args::try_parse_from([
            "civ6-tuner",
            "lua",
            "--state",
            "-1",
            "--code",
            "-2.5",
            "--c",
            "return 1",
        ])
        .unwrap();
        assert_eq!(args.state, "-1");
        assert_eq!(args.code.as_deref(), Some("return 1"));
        for flag in ["--state", "--code"] {
            for value in ["--unknown", "-not-a-number", "-1file"] {
                parser_tests::assert_parse_error::<Args>(&[
                    "civ6-tuner",
                    "lua",
                    flag,
                    value,
                    flag,
                    "InGame",
                ]);
            }
        }
    }

    #[test]
    fn compat_tuner_prefixes_match_argparse() {
        parser_tests::assert_long_prefixes::<Args>(
            &["civ6-tuner", "lua"],
            &[
                ("--state", &["InGame"]),
                ("--code", &["return 1"]),
                ("--port", &["4318"]),
            ],
        );
    }

    #[test]
    fn compat_tuner_last_values_preserve_empty_lua_and_state() {
        let args = Args::try_parse_from([
            "civ6-tuner",
            "lua",
            "--state",
            "InGame",
            "--s",
            "",
            "--code",
            "return 1",
            "--c=",
            "--port",
            "65536",
            "--p",
            "4318",
        ])
        .unwrap();
        assert!(matches!(args.command, Command::Lua));
        assert_eq!(args.state, "");
        assert_eq!(args.code.as_deref(), Some(""));
        assert_eq!(args.port.socket_port().unwrap(), 4318);
    }

    #[test]
    fn compat_tuner_commands_and_each_numeric_occurrence_stay_strict() {
        for args in [
            vec!["civ6-tuner", "sta"],
            vec!["civ6-tuner", "lu"],
            vec!["civ6-tuner", "LUA"],
            vec!["civ6-tuner", "lua", "--port", "", "--p", "4318"],
            vec!["civ6-tuner", "lua", "--port", "1.5", "--p", "4318"],
            vec!["civ6-tuner", "lua", "--state-extra", "InGame"],
        ] {
            parser_tests::assert_parse_error::<Args>(&args);
        }
    }
}
