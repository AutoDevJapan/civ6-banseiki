use std::{collections::BTreeSet, env, fs, path::Path};
#[path = "src/spy_completed_failure.rs"]
mod spy_completed_failure;
#[path = "src/spy_third.rs"]
mod spy_third;
#[path = "src/spy_preparation_install.rs"]
mod spy_preparation_install;

fn collect_ids(dir: &Path, ids: &mut BTreeSet<String>) {
    println!("cargo:rerun-if-changed={}", dir.display());
    for entry in fs::read_dir(dir).expect("read repository MOD directory") {
        let entry = entry.expect("read MOD entry");
        let path = entry.path();
        let kind = entry.file_type().expect("read MOD entry type");
        if kind.is_dir() {
            collect_ids(&path, ids);
        } else if kind.is_file()
            && path.extension().is_some_and(|v| v == "modinfo")
            && entry.file_name().to_string_lossy().starts_with("JAC_")
        {
            let text = fs::read_to_string(&path).expect("read MOD metadata");
            let doc = roxmltree::Document::parse(text.trim_start_matches('\u{feff}'))
                .expect("parse MOD metadata");
            let root = doc.root_element();
            assert!(
                root.has_tag_name("Mod"),
                "invalid MOD root: {}",
                path.display()
            );
            let id = root.attribute("id").expect("MOD id").to_ascii_lowercase();
            assert!(!id.is_empty(), "empty MOD id: {}", path.display());
            ids.insert(id);
        }
    }
}

fn main() {
    let manifest = env::var_os("CARGO_MANIFEST_DIR").expect("manifest directory");
    println!("cargo:rerun-if-changed=src/spy_third.rs");
    println!("cargo:rerun-if-changed=src/spy_completed_failure.rs");
    println!("cargo:rerun-if-changed=src/spy_returned_preparation.rs");
    println!("cargo:rerun-if-changed=src/spy_preparation_install.rs");
    spy_preparation_install::write(&Path::new(&manifest).join("../.."))
        .expect("generate test-only returned preparation event connection");
    spy_completed_failure::write(&Path::new(&manifest).join("../.."))
        .expect("generate shared native completed-failure policy from Rust");
    spy_third::write_connection(&Path::new(&manifest).join("../.."), false)
        .expect("generate test-only third spy connection from Rust");
    let mut ids = BTreeSet::new();
    collect_ids(&Path::new(&manifest).join("../../poc"), &mut ids);
    assert!(!ids.is_empty(), "no repository MOD ids");
    let source = format!("&{:?}", ids.into_iter().collect::<Vec<_>>());
    fs::write(
        Path::new(&env::var_os("OUT_DIR").expect("build output")).join("own_mod_ids.rs"),
        source,
    )
    .expect("write embedded MOD ids");
}
