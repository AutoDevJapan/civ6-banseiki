//! The legacy argparse option boundary, with Clap still owning values and guards.
use clap::{error::ErrorKind, Arg, ArgAction, Command, Error, Parser};
use regex::Regex;
use std::{ffi::OsString, sync::OnceLock};

fn negative_number(value: &std::ffi::OsStr) -> bool {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    value.to_str().is_some_and(|value| {
        // Python's Unicode \d means decimal digits; its $ also permits a final LF.
        PATTERN
            .get_or_init(|| Regex::new(r"\A-(?:\p{Nd}+|\p{Nd}*\.\p{Nd}+)\z").unwrap())
            .is_match(value.strip_suffix('\n').unwrap_or(value))
    })
}

fn option_like(value: &std::ffi::OsStr) -> bool {
    let bytes = value.as_encoded_bytes();
    bytes.len() > 1 && bytes[0] == b'-' && !negative_number(value)
}

fn boundary_error(kind: ErrorKind, value: &std::ffi::OsStr) -> Error {
    Error::raw(
        kind,
        format!("unexpected option or missing value before {:?}", value),
    )
}

fn option<'a>(scope: &'a Command, token: &std::ffi::OsStr) -> Result<(&'a Arg, bool), Error> {
    let bytes = token.as_encoded_bytes();
    let end = bytes
        .iter()
        .position(|&byte| byte == b'=')
        .unwrap_or(bytes.len());
    let attached = end != bytes.len();
    let name = std::str::from_utf8(&bytes[..end])
        .map_err(|_| boundary_error(ErrorKind::UnknownArgument, token))?;
    if let Some(long) = name.strip_prefix("--") {
        let names = |arg: &'a Arg| {
            arg.get_long()
                .into_iter()
                .chain(arg.get_all_aliases().unwrap_or_default())
        };
        if let Some(arg) = scope
            .get_arguments()
            .find(|arg| names(arg).any(|name| name == long))
        {
            return Ok((arg, attached));
        }
        {
            let mut candidates = scope
                .get_arguments()
                .filter(|arg| names(arg).any(|name| name.starts_with(long)));
            if let Some(arg) = candidates.next() {
                if candidates.next().is_none() {
                    return Ok((arg, attached));
                }
            }
        }
    } else if let Some(short) = name
        .strip_prefix('-')
        .filter(|name| name.chars().count() == 1)
    {
        let short = short.chars().next().unwrap();
        if let Some(arg) = scope.get_arguments().find(|arg| {
            arg.get_short() == Some(short)
                || arg
                    .get_all_short_aliases()
                    .unwrap_or_default()
                    .contains(&short)
        }) {
            return Ok((arg, attached));
        }
    }
    Err(boundary_error(ErrorKind::UnknownArgument, token))
}

fn validate_boundaries(command: &Command, tokens: &[OsString]) -> Result<(), Error> {
    let mut scope = command;
    let mut pending: Option<(&Arg, usize)> = None;
    for token in tokens.iter().skip(1) {
        let boundary = token == "--" || option_like(token);
        if let Some((arg, taken)) = pending.as_mut() {
            let arity = arg.get_num_args().unwrap();
            if !boundary {
                *taken += 1;
                if *taken == arity.max_values() {
                    pending = None;
                }
                continue;
            }
            if *taken < arity.min_values() {
                return Err(boundary_error(ErrorKind::InvalidValue, token));
            }
            pending = None;
        }
        if token == "--" {
            return Ok(());
        }
        if boundary {
            let (arg, attached) = option(scope, token)?;
            let arity = arg.get_num_args().unwrap();
            if attached && arity.max_values() == 0 {
                return Err(boundary_error(ErrorKind::TooManyValues, token));
            }
            if matches!(
                arg.get_action(),
                ArgAction::Help | ArgAction::HelpShort | ArgAction::HelpLong | ArgAction::Version
            ) {
                return Ok(());
            }
            let taken = usize::from(attached);
            if taken < arity.max_values() {
                pending = Some((arg, taken));
            }
        } else if let Some(child) = scope.get_subcommands().find(|child| {
            token == child.get_name() || child.get_all_aliases().any(|alias| token == alias)
        }) {
            scope = child;
        }
    }
    if let Some((arg, taken)) = pending {
        if taken < arg.get_num_args().unwrap().min_values() {
            return Err(Error::raw(
                ErrorKind::InvalidValue,
                format!("a value is required for {}", arg.get_id()),
            ));
        }
    }
    Ok(())
}

fn value_grammar(command: Command) -> Command {
    let mut command = command.mut_args(|arg| {
        // Fixed scalar/rectangle/positional arities cannot absorb a following flag
        // after validation. Variadic choice lists retain Clap's flag boundary.
        let fixed = arg
            .get_num_args()
            .is_some_and(|arity| arity.max_values() != usize::MAX);
        let values = arg.get_action().takes_values();
        arg.allow_negative_numbers(false)
            .allow_hyphen_values(values && fixed)
    });
    for child in command.get_subcommands_mut() {
        *child = value_grammar(child.clone());
    }
    command
}

pub fn try_parse_from<P, I, T>(args: I) -> Result<P, Error>
where
    P: Parser,
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let tokens: Vec<OsString> = args.into_iter().map(Into::into).collect();
    let mut command = P::command().infer_long_args(true);
    command.build();
    validate_boundaries(&command, &tokens)?;
    let mut matches = value_grammar(command).try_get_matches_from(tokens)?;
    P::from_arg_matches_mut(&mut matches)
}

/// Route inherent calls and real CLI entry points through the same boundary.
#[macro_export]
macro_rules! parser {
    ($parser:ty) => {
        impl $parser {
            pub fn try_parse_from<I, T>(args: I) -> ::std::result::Result<Self, ::clap::Error>
            where
                I: IntoIterator<Item = T>,
                T: Into<::std::ffi::OsString> + Clone,
            {
                $crate::try_parse_from::<Self, I, T>(args)
            }
            pub fn parse() -> Self {
                Self::try_parse_from(::std::env::args_os()).unwrap_or_else(|error| error.exit())
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Subcommand;
    use std::ffi::OsString;

    #[derive(Debug, Parser)]
    #[command(args_override_self = true)]
    struct Args {
        path: OsString,
        #[arg(long)]
        state: Option<String>,
        #[arg(long)]
        code: Option<String>,
        #[arg(long, value_parser = ["generic", "growth"])]
        profile: Option<String>,
        #[arg(long)]
        output: Option<OsString>,
        #[arg(long)]
        owner_start: bool,
        #[arg(long)]
        check: bool,
        #[arg(long, num_args = 4, action = clap::ArgAction::Set)]
        rect: Option<Vec<f64>>,
        #[arg(long)]
        number: Option<i64>,
        #[arg(long, num_args = 1.., action = clap::ArgAction::Set, value_parser = ["A", "B"])]
        only: Option<Vec<String>>,
    }
    crate::parser!(Args);

    fn fails(args: &[&str]) {
        assert_eq!(
            Args::try_parse_from(args).unwrap_err().exit_code(),
            2,
            "{args:?}"
        );
    }

    #[test]
    fn compat_shared_legacy_negative_strings_paths_and_unicode_digits() {
        for value in ["-1", "-.5", "-1.5", "-٣", "-.٥"] {
            let args = Args::try_parse_from([
                "cli", value, "--state", value, "--s", "InGame", "--code", value, "--output",
                value, "--check", "--ch",
            ])
            .unwrap();
            assert_eq!(args.path, OsString::from(value));
            assert_eq!(args.state.as_deref(), Some("InGame"));
            assert_eq!(args.code.as_deref(), Some(value));
            assert_eq!(args.output, Some(OsString::from(value)));
            assert!(args.check);
        }
    }

    #[test]
    fn compat_shared_rejects_separated_exponents_trailing_dots_and_flags() {
        for value in ["-1e3", "-1.", "--unknown", "-not-a-number"] {
            for flag in ["--state", "--code", "--output"] {
                fails(&["cli", "file", flag, value, flag, "last"]);
            }
            fails(&["cli", value]);
        }
        for tail in [
            vec!["--state", "--check"],
            vec!["--state", "--help"],
            vec!["--state"],
            vec!["--o", "path"],
            vec!["--check=true"],
        ] {
            let mut args = vec!["cli", "file"];
            args.extend(tail);
            fails(&args);
        }
    }

    #[test]
    fn compat_shared_attached_values_and_explicit_separator_keep_original_strings() {
        let args = Args::try_parse_from([
            "cli",
            "file",
            "--state=-1e3",
            "--code=-1.",
            "--output=--unknown",
        ])
        .unwrap();
        assert_eq!(args.state.as_deref(), Some("-1e3"));
        assert_eq!(args.code.as_deref(), Some("-1."));
        assert_eq!(args.output, Some(OsString::from("--unknown")));
        let args = Args::try_parse_from(["cli", "--state=", "--", "-1e3"]).unwrap();
        assert_eq!(args.state.as_deref(), Some(""));
        assert_eq!(args.path, OsString::from("-1e3"));
    }

    #[test]
    fn compat_shared_rect_arity_choices_and_numeric_casts_remain_strict() {
        let args = Args::try_parse_from([
            "cli", "file", "--rect", "-.5", "0", "1", "1", "--rect", "0", "0", "1", "1",
        ])
        .unwrap();
        assert_eq!(args.rect, Some(vec![0.0, 0.0, 1.0, 1.0]));
        for value in ["-1e3", "-1.", "--check"] {
            fails(&["cli", "file", "--rect", value, "0", "1", "1"]);
        }
        fails(&["cli", "file", "--rect", "-.5", "0", "1", "--check"]);
        fails(&["cli", "file", "--number", "-.5"]);
        let error =
            Args::try_parse_from(["cli", "file", "--profile", "-.5", "--profile", "generic"])
                .unwrap_err();
        assert_eq!(error.kind(), ErrorKind::InvalidValue);
    }

    #[test]
    fn compat_shared_help_and_variadic_choice_boundaries_keep_flags() {
        let _entrypoint: fn() -> Args = Args::parse;
        for help in ["-h", "--help", "--he"] {
            assert_eq!(
                Args::try_parse_from(["cli", help]).unwrap_err().exit_code(),
                0
            );
        }
        let args =
            Args::try_parse_from(["cli", "file", "--only", "A", "B", "--check", "--only", "A"])
                .unwrap();
        assert_eq!(args.only, Some(vec!["A".into()]));
        assert!(args.check);
        fails(&["cli", "file", "--only", "--check"]);
        fails(&["cli", "file", "--only", "A", "--unknown"]);
    }

    #[derive(Debug, Parser)]
    #[command(args_override_self = true)]
    struct Root {
        #[command(subcommand)]
        command: Action,
    }
    #[derive(Debug, Subcommand)]
    enum Action {
        Wait {
            #[arg(long)]
            seconds: f64,
        },
    }
    crate::parser!(Root);

    #[test]
    fn compat_shared_subcommand_scope_keeps_values_and_unknown_flags_separate() {
        let _entrypoint: fn() -> Root = Root::parse;
        let args = Root::try_parse_from(["cli", "wait", "--seconds", "-.5", "--s", "0"]).unwrap();
        assert!(matches!(args.command, Action::Wait { seconds } if seconds == 0.0));
        for args in [
            vec!["cli", "wait", "--seconds", "-1e3"],
            vec!["cli", "wait", "--state", "x"],
            vec!["cli", "wa"],
        ] {
            assert_eq!(Root::try_parse_from(&args).unwrap_err().exit_code(), 2);
        }
    }

    #[cfg(windows)]
    #[test]
    fn compat_shared_preserves_native_path_bytes_in_attached_and_positional_values() {
        use std::os::windows::ffi::OsStringExt;
        let path = OsString::from_wide(&[b'x' as u16, 0xd800]);
        let mut output = OsString::from("--output=");
        output.push(&path);
        let args = Args::try_parse_from([OsString::from("cli"), path.clone(), output]).unwrap();
        assert_eq!(args.path, path);
        assert_eq!(args.output, Some(path));
    }
}
