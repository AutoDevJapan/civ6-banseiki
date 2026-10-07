//! Regenerate poc/JAC_PoC_BanseikiPool from documents/spec/data/banseiki-pool.json.
//! `--check` writes nothing and fails when a checked-in file differs from the generator.
fn main() -> anyhow::Result<()> {
    let check = std::env::args().skip(1).any(|arg| arg == "--check");
    civ6_assets_rs::banseiki_pool::run(check)
}
