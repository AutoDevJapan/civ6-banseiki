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

#[derive(Clone, Copy)]
enum Optional<'a> {
    Value,
    Known(&'a Arg, bool),
    Unknown,
}

fn unmatched<'a>(token: &std::ffi::OsStr) -> Optional<'a> {
    // argparse checks known options and ambiguous prefixes before this fallback.
    if negative_number(token) || token.as_encoded_bytes().contains(&b' ') {
        Optional::Value
    } else {
        Optional::Unknown
    }
}

fn boundary_error(kind: ErrorKind, value: &std::ffi::OsStr) -> Error {
    Error::raw(
        kind,
        format!("unexpected option or missing value before {:?}", value),
    )
}

fn option<'a>(scope: &'a Command, token: &std::ffi::OsStr) -> Result<Optional<'a>, Error> {
    let bytes = token.as_encoded_bytes();
    if bytes.len() <= 1 || bytes[0] != b'-' {
        return Ok(Optional::Value);
    }
    let end = bytes
        .iter()
        .position(|&byte| byte == b'=')
        .unwrap_or(bytes.len());
    let attached = end != bytes.len();
    let Ok(name) = std::str::from_utf8(&bytes[..end]) else {
        return Ok(unmatched(token));
    };
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
            return Ok(Optional::Known(arg, attached));
        }
        {
            let mut candidates = scope
                .get_arguments()
                .filter(|arg| names(arg).any(|name| name.starts_with(long)));
            if let Some(arg) = candidates.next() {
                if candidates.next().is_none() {
                    return Ok(Optional::Known(arg, attached));
                }
                return Err(Error::raw(
                    ErrorKind::UnknownArgument,
                    format!("ambiguous option {token:?}"),
                ));
            }
        }
    } else if let Some(short_name) = name.strip_prefix('-') {
        let mut chars = short_name.chars();
        let short = chars.next().unwrap();
        let attached = attached || chars.next().is_some();
        if let Some(arg) = scope.get_arguments().find(|arg| {
            arg.get_short() == Some(short)
                || arg
                    .get_all_short_aliases()
                    .unwrap_or_default()
                    .contains(&short)
        }) {
            return Ok(Optional::Known(arg, attached));
        }
    }
    Ok(unmatched(token))
}

fn validate_boundaries(command: &Command, tokens: &[OsString]) -> Result<Vec<OsString>, Error> {
    let mut scope = command;
    let mut pending: Option<(&Arg, usize)> = None;
    let mut unknown = None;
    let mut accepted: Vec<_> = tokens.iter().take(1).cloned().collect();
    for (index, token) in tokens.iter().enumerate().skip(1) {
        let parsed = if token == "--" {
            Optional::Unknown
        } else {
            option(scope, token)?
        };
        let boundary = !matches!(parsed, Optional::Value);
        if let Some((arg, taken)) = pending.as_mut() {
            let arity = arg.get_num_args().unwrap();
            if !boundary {
                accepted.push(token.clone());
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
            accepted.extend_from_slice(&tokens[index..]);
            return unknown.map_or(Ok(accepted), Err);
        }
        if let Optional::Known(arg, attached) = parsed {
            accepted.push(token.clone());
            let arity = arg.get_num_args().unwrap();
            if attached && arity.max_values() == 0 {
                return Err(boundary_error(ErrorKind::TooManyValues, token));
            }
            if matches!(
                arg.get_action(),
                ArgAction::Help | ArgAction::HelpShort | ArgAction::HelpLong | ArgAction::Version
            ) {
                // Drop only deferred unknown flags. Clap still checks preceding
                // typed values before printing this scope's normal help text.
                return Ok(accepted);
            }
            let taken = usize::from(attached);
            if taken < arity.max_values() {
                pending = Some((arg, taken));
            }
        } else if matches!(parsed, Optional::Unknown) {
            unknown.get_or_insert_with(|| boundary_error(ErrorKind::UnknownArgument, token));
        } else {
            accepted.push(token.clone());
            if let Some(child) = scope.get_subcommands().find(|child| {
                token == child.get_name() || child.get_all_aliases().any(|alias| token == alias)
            }) {
                scope = child;
            }
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
    unknown.map_or(Ok(accepted), Err)
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
    let tokens = validate_boundaries(&command, &tokens)?;
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
    fn compat_shared_space_containing_values_keep_lua_and_path_bytes() {
        for value in [
            "-- comment\nreturn 1",
            "- path with spaces",
            "--unknown=two words",
        ] {
            let args = Args::try_parse_from([
                "cli", value, "--state", value, "--code", value, "--output", value,
            ])
            .unwrap();
            assert_eq!(args.path, OsString::from(value));
            assert_eq!(args.state.as_deref(), Some(value));
            assert_eq!(args.code.as_deref(), Some(value));
            assert_eq!(args.output, Some(OsString::from(value)));
        }
        let args = Args::try_parse_from(["cli", "file", "--code=-- comment\nreturn 1"]).unwrap();
        assert_eq!(args.code.as_deref(), Some("-- comment\nreturn 1"));
    }

    #[test]
    fn compat_shared_known_options_and_ambiguity_precede_space_fallback() {
        for args in [
            vec!["cli", "file", "--state", "--code=two words"],
            vec!["cli", "file", "--state", "--output=two words"],
            vec!["cli", "file", "--o=two words", "--help"],
            vec!["cli", "file", "--state", "--unknown"],
            vec!["cli", "file", "--code", "--comment\nreturn1"],
            vec!["cli", "file", "--code", "-\tpath"],
            vec!["cli", "file", "-h two words"],
        ] {
            fails(&args);
        }
    }

    #[test]
    fn compat_shared_help_overrides_unknowns_but_not_missing_or_invalid_values() {
        for help in ["-h", "--help", "--he"] {
            let error = Args::try_parse_from(["cli", "--unknown", help]).unwrap_err();
            assert_eq!(error.kind(), ErrorKind::DisplayHelp);
            assert_eq!(error.exit_code(), 0);
            assert!(error.to_string().contains("--state"));
        }
        for args in [
            vec!["cli", "--unknown", "--state", "--help"],
            vec!["cli", "--state", "--help"],
            vec!["cli", "file", "--profile", "invalid", "--unknown", "--help"],
            vec!["cli", "file", "--unknown"],
            vec!["cli", "file", "--unknown", "--", "--help"],
        ] {
            fails(&args);
        }
        let error = Root::try_parse_from(["cli", "wait", "--unknown", "--help"]).unwrap_err();
        assert_eq!(error.exit_code(), 0);
        assert!(error.to_string().contains("--seconds"));
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
        let path = OsString::from_wide(&[b'-' as u16, b' ' as u16, b'x' as u16, 0xd800]);
        let args = Args::try_parse_from([
            OsString::from("cli"),
            path.clone(),
            OsString::from("--output"),
            path.clone(),
        ])
        .unwrap();
        assert_eq!(args.path, path);
        assert_eq!(args.output, Some(path));
    }
}
