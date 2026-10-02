use clap::Parser;

pub fn assert_long_prefixes<P: Parser>(base: &[&str], options: &[(&str, &[&str])]) {
    // Keep this inventory explicit: these are the argparse flags (plus new flags),
    // not names discovered from the Clap parser under test.
    let options: Vec<_> = options
        .iter()
        .copied()
        .chain(std::iter::once(("--help", &[][..])))
        .collect();
    for (name, values) in &options {
        for end in 3..=name.len() {
            let prefix = &name[..end];
            let mut args = base.to_vec();
            args.push(prefix);
            args.extend_from_slice(values);
            let result = civ6_cli_compat_rs::try_parse_from::<P, _, _>(args);
            let exact = options.iter().any(|(flag, _)| *flag == prefix);
            let matches = options
                .iter()
                .filter(|(flag, _)| flag.starts_with(prefix))
                .count();
            if !exact && matches > 1 {
                let error = result.err().unwrap_or_else(|| panic!("accepted {prefix}"));
                assert_eq!(error.exit_code(), 2, "{prefix}: {error}");
            } else if *name == "--help" {
                let error = result.err().unwrap_or_else(|| panic!("ignored {prefix}"));
                assert_eq!(error.kind(), clap::error::ErrorKind::DisplayHelp);
                assert_eq!(error.exit_code(), 0);
            } else {
                assert!(result.is_ok(), "{prefix}: {}", result.err().unwrap());
            }
        }
    }
}

pub fn assert_parse_error<P: Parser>(args: &[&str]) {
    let error = civ6_cli_compat_rs::try_parse_from::<P, _, _>(args)
        .err()
        .unwrap_or_else(|| panic!("accepted {args:?}"));
    assert_eq!(error.exit_code(), 2, "{args:?}: {error}");
}
