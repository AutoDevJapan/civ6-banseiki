fn main() {
    cc_live::process::install_interrupt_handler();
    let result = cc_live::chain::cli(
        &mut cc_live::process::RealRunner,
        &cc_live::Paths::default(),
        &std::env::args().skip(1).collect::<Vec<_>>(),
    );
    match result {
        Ok(code) => std::process::exit(cc_live::chain::python_cli_exit(code)),
        Err(e) => {
            use std::io::Write;
            let _ = writeln!(std::io::stderr().lock(), "{e}");
            std::process::exit(1);
        }
    }
}
