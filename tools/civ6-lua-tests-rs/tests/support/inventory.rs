use std::collections::BTreeSet;
use std::path::Path;
use syn::parse::Parser;
use syn::{punctuated::Punctuated, Ident, Item, Token};

// This is the sole test-generating macro in the migrated suites. Validate the
// definition as tokens as well as parsing its invocations; a name list alone
// must not count as registered tests if the expansion stops producing tests.
const SIMPLE_TESTS: &str = r#"
macro_rules! simple_tests {
    ($($name:ident),* $(,)?) => {
        $(#[test] fn $name() { Harness::new().case(stringify!($name)); })*
    };
}
"#;

fn has_attr(attrs: &[syn::Attribute], name: &str) -> bool {
    attrs.iter().any(|a| a.path().is_ident(name))
}

pub fn parse(source: &str, prefix: &str) -> BTreeSet<String> {
    let file = syn::parse_file(source).expect("invalid Rust test source");
    let mut tests = BTreeSet::new();
    walk(&file.items, prefix, None, &mut tests);
    tests
}

pub fn resolve(path: &Path, prefix: &str) -> BTreeSet<String> {
    let file = syn::parse_file(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut tests = BTreeSet::new();
    walk(
        &file.items,
        prefix,
        Some(path.parent().unwrap()),
        &mut tests,
    );
    tests
}

fn walk(items: &[Item], prefix: &str, directory: Option<&Path>, tests: &mut BTreeSet<String>) {
    for item in items {
        match item {
            Item::Fn(f) if has_attr(&f.attrs, "test") => {
                assert!(
                    !has_attr(&f.attrs, "ignore"),
                    "ignored test {prefix}::{}",
                    f.sig.ident
                );
                assert!(
                    !has_attr(&f.attrs, "cfg"),
                    "conditional test {prefix}::{}",
                    f.sig.ident
                );
                assert!(
                    tests.insert(format!("{prefix}::{}", f.sig.ident)),
                    "duplicate test"
                );
            }
            Item::Mod(module) => {
                assert!(
                    !has_attr(&module.attrs, "cfg"),
                    "conditional module {prefix}::{}",
                    module.ident
                );
                let nested = format!("{prefix}::{}", module.ident);
                if let Some((_, items)) = &module.content {
                    let dir = directory.map(|d| d.join(module.ident.to_string()));
                    walk(items, &nested, dir.as_deref(), tests);
                } else if let Some(directory) = directory {
                    let override_path = module.attrs.iter().find_map(|attr| {
                        if !attr.path().is_ident("path") {
                            return None;
                        }
                        let syn::Meta::NameValue(meta) = &attr.meta else {
                            panic!("invalid module path")
                        };
                        let syn::Expr::Lit(lit) = &meta.value else {
                            panic!("invalid module path")
                        };
                        let syn::Lit::Str(path) = &lit.lit else {
                            panic!("invalid module path")
                        };
                        Some(path.value())
                    });
                    let path = if let Some(path) = override_path {
                        directory.join(path)
                    } else {
                        let file = directory.join(format!("{}.rs", module.ident));
                        if file.is_file() {
                            file
                        } else {
                            directory.join(module.ident.to_string()).join("mod.rs")
                        }
                    };
                    let parsed = syn::parse_file(&std::fs::read_to_string(&path).unwrap()).unwrap();
                    let child_dir = if path.file_name().unwrap() == "mod.rs" {
                        path.parent().unwrap().to_path_buf()
                    } else {
                        path.parent().unwrap().join(path.file_stem().unwrap())
                    };
                    walk(&parsed.items, &nested, Some(&child_dir), tests);
                }
            }
            Item::Macro(mac)
                if mac.mac.path.is_ident("macro_rules")
                    && mac.ident.as_ref().is_some_and(|i| i == "simple_tests") =>
            {
                let expected: syn::ItemMacro = syn::parse_str(SIMPLE_TESTS).unwrap();
                assert_eq!(
                    mac.mac.tokens.to_string(),
                    expected.mac.tokens.to_string(),
                    "unverified test macro expansion"
                );
            }
            Item::Macro(mac) if mac.mac.path.is_ident("simple_tests") => {
                assert!(
                    !has_attr(&mac.attrs, "cfg") && !has_attr(&mac.attrs, "ignore"),
                    "conditional test macro {prefix}"
                );
                let names = Punctuated::<Ident, Token![,]>::parse_terminated
                    .parse2(mac.mac.tokens.clone())
                    .unwrap();
                assert!(!names.is_empty(), "empty test macro");
                for name in names {
                    assert!(
                        tests.insert(format!("{prefix}::{name}")),
                        "duplicate macro test"
                    );
                }
            }
            _ => {}
        }
    }
}
