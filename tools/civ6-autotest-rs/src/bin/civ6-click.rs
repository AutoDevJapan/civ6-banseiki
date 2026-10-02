use civ6_autotest_rs::click;
use clap::Parser;

#[derive(Parser)]
#[command(
    about = "Send verified input to the one Civ VI window",
    infer_long_args = true,
    args_override_self = true
)]
struct Args {
    /// Horizontal fraction of the window (0..1).
    fx: Option<f64>,
    /// Vertical fraction of the window (0..1).
    fy: Option<f64>,
    /// Post a virtual key instead of clicking.
    #[arg(long)]
    key: Option<u16>,
    /// Request the Civ VI window to close.
    #[arg(long)]
    close: bool,
    /// Absolute monotonic deadline for --key.
    #[arg(long)]
    deadline: Option<f64>,
}

fn main() {
    let args = Args::parse();
    let result = match (args.fx, args.fy, args.key, args.close) {
        (Some(x), Some(y), None, false) => {
            click::click_fraction(x, y).map(|(x, y)| format!("({x}, {y})"))
        }
        (None, None, Some(vk), false) => {
            click::press_key(vk, args.deadline).map(|_| "key sent".to_string())
        }
        (None, None, None, true) => {
            click::request_civ_window_close().map(|_| "close requested".to_string())
        }
        _ => Err(civ6_autotest_rs::Error::input(
            "Supply fx fy, --key VK, or --close",
        )),
    };
    match result {
        Ok(message) => println!("{message}"),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}

#[cfg(test)]
#[path = "support/parser_tests.rs"]
mod parser_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compat_click_negative_numbers_do_not_accept_arbitrary_hyphen_values() {
        let args = Args::try_parse_from([
            "civ6-click",
            "-0.5",
            "0.5",
            "--deadline",
            "-1.5",
            "--d",
            "123",
        ])
        .unwrap();
        assert_eq!((args.fx, args.fy), (Some(-0.5), Some(0.5)));
        assert_eq!(args.deadline, Some(123.0));
        parser_tests::assert_parse_error::<Args>(&[
            "civ6-click",
            "--deadline",
            "--unknown",
            "--d",
            "123",
        ]);
    }

    #[test]
    fn compat_click_accepts_long_prefixes() {
        parser_tests::assert_long_prefixes::<Args>(
            &["civ6-click"],
            &[
                ("--key", &["27"]),
                ("--close", &[]),
                ("--deadline", &["123"]),
            ],
        );
    }

    #[test]
    fn compat_click_last_values_do_not_override_other_operation_flags() {
        let args = Args::try_parse_from([
            "civ6-click",
            "0.5",
            "0.6",
            "--key",
            "1",
            "--k",
            "27",
            "--deadline",
            "100",
            "--d",
            "123",
            "--close",
            "--cl",
        ])
        .unwrap();
        assert_eq!((args.fx, args.fy), (Some(0.5), Some(0.6)));
        assert_eq!(args.key, Some(27));
        assert_eq!(args.deadline, Some(123.0));
        assert!(args.close);
    }

    #[test]
    fn compat_click_numbers_and_bool_values_stay_strict() {
        for args in [
            vec!["civ6-click", "--key", "", "--k", "27"],
            vec!["civ6-click", "--key", "65536", "--k", "27"],
            vec!["civ6-click", "--deadline", "", "--d", "123"],
            vec!["civ6-click", "--close=true"],
            vec!["civ6-click", "--close-extra"],
            vec!["civ6-click", "", "0.5"],
        ] {
            parser_tests::assert_parse_error::<Args>(&args);
        }
    }
}

civ6_cli_compat_rs::parser!(Args);
