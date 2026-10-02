fn main() {
    let repo = civ6_data_rs::Repository::checked_in();
    let args: Vec<_> = std::env::args().skip(1).collect();
    if let Err(error) = civ6_data_rs::cascade::cli(&repo, &args) {
        eprintln!("{error:#}");
        std::process::exit(civ6_data_rs::cli_exit_code(&error));
    }
}
