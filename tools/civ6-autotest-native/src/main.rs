mod logs;
mod mods;
mod platform;
mod runner;
mod scripts;
mod tuner;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match runner::cli(&args) {
        Ok(code) => std::process::exit(code),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
