fn main() -> std::process::ExitCode {
    let result = civ6_autotest_rs::steam_dialog::Session::begin()
        .and_then(|s| civ6_autotest_rs::steam_dialog::probe(&s));
    match result {
        Ok(value) => {
            println!("{value}");
            std::process::ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("{e}");
            std::process::ExitCode::FAILURE
        }
    }
}
