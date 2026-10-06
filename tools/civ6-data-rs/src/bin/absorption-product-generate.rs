use civ6_data_rs::{absorption_product, apply_outputs, parse_check_args, Repository};
fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let Some(check)=parse_check_args("absorption-product-generate",&args,"Generate ordinary product leader terms, Gameplay operations and four backend acceptance scenes; UI116 is pending.")? else{return Ok(())};
    let repo = Repository::checked_in();
    apply_outputs(&repo, absorption_product::generate(&repo)?, check)
}
