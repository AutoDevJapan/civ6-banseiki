use civ6_autotest_rs::capture;
use clap::Parser;
use std::path::PathBuf;

mod support;

#[derive(Parser)]
#[command(
    about = "Capture the verified Civ VI window as PNG",
    infer_long_args = true,
    args_override_self = true
)]
struct Args {
    #[arg(value_parser = support::path_value_parser())]
    path: PathBuf,
    /// Crop fractions left top right bottom.
    #[arg(long, num_args = 4, action = clap::ArgAction::Set)]
    crop: Option<Vec<f64>>,
    /// Absolute monotonic deadline.
    #[arg(long)]
    deadline: Option<f64>,
}

fn main() {
    let args = Args::parse();
    let crop = args.crop.map(|v| [v[0], v[1], v[2], v[3]]);
    if let Err(error) = capture::capture_civ_window(&args.path, crop, args.deadline) {
        eprintln!("{error}");
        std::process::exit(1);
    }
    println!("{}", args.path.display());
}

#[cfg(test)]
#[path = "support/parser_tests.rs"]
mod parser_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compat_capture_accepts_long_prefixes() {
        parser_tests::assert_long_prefixes::<Args>(
            &["civ6-capture", "shot.png"],
            &[("--crop", &["0", "0", "1", "1"]), ("--deadline", &["123"])],
        );
    }

    #[test]
    fn compat_capture_replaces_the_whole_crop_occurrence_and_empty_path() {
        let args = Args::try_parse_from([
            "civ6-capture",
            "",
            "--crop",
            "0",
            "0",
            "1",
            "1",
            "--c",
            "0.1",
            "0.2",
            "0.8",
            "0.9",
            "--deadline",
            "100",
            "--d",
            "123",
        ])
        .unwrap();
        assert_eq!(args.path, PathBuf::from("."));
        assert_eq!(args.crop, Some(vec![0.1, 0.2, 0.8, 0.9]));
        assert_eq!(args.deadline, Some(123.0));
    }

    #[test]
    fn compat_capture_validates_every_crop_value_and_occurrence_length() {
        for tail in [
            vec!["--crop", "0", "0", "1", "--c", "0", "0", "1", "1"],
            vec!["--crop", "0", "0", "", "1", "--c", "0", "0", "1", "1"],
            vec!["--crop", "0", "0", "1", "1", "--c", "0", "0", "1"],
            vec!["--deadline", "", "--d", "123"],
            vec!["--crop-extra", "0", "0", "1", "1"],
        ] {
            let mut args = vec!["civ6-capture", "shot.png"];
            args.extend(tail);
            parser_tests::assert_parse_error::<Args>(&args);
        }
    }
}
