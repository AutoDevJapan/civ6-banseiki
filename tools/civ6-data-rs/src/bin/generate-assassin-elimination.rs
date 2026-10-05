fn main() {
    let repo = civ6_data_rs::Repository::checked_in();
    let args: Vec<_> = std::env::args().skip(1).collect();
    let result = civ6_data_rs::parse_check_args(
        "generate-assassin-elimination",
        &args,
        "Generate native actor/target country elimination save/load scenarios (ToDo #172)",
    )
    .and_then(|check| match check {
        Some(check) => civ6_data_rs::assassin::elimination::cli(&repo, check),
        None => Ok(()),
    });
    if let Err(error) = result {
        eprintln!("{error:#}");
        std::process::exit(civ6_data_rs::cli_exit_code(&error));
    }
}
