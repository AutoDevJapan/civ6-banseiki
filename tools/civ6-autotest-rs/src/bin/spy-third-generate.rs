//! Regenerate the bounded third spy connection without running Civ6 or Cargo.
#[path = "../spy_third.rs"]
mod spy_third;

fn main() -> std::io::Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let root = std::path::Path::new(
        args.first()
            .expect("usage: spy-third-generate ROOT [--check]"),
    );
    spy_third::write_connection(root, args.get(1).is_some_and(|v| v == "--check"))
}
