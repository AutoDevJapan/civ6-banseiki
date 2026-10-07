fn main() {
    let result = cc_live::review::merge_cli(
        &mut cc_live::process::RealRunner,
        &cc_live::Paths::default(),
        &std::env::args().skip(1).collect::<Vec<_>>(),
    );
    match result {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            use std::io::Write;
            let _ = writeln!(std::io::stderr().lock(), "{e}");
            std::process::exit(1);
        }
    }
}
