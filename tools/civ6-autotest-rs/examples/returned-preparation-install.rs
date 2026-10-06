#[path = "../src/spy_preparation_install.rs"]
mod installer;
fn main() -> std::io::Result<()> {
    let root = std::env::args_os()
        .nth(1)
        .ok_or_else(|| std::io::Error::other("repository root required"))?;
    installer::write(std::path::Path::new(&root))
}
