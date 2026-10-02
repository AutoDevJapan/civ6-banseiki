use proc_macro2::{TokenStream, TokenTree};
use std::collections::BTreeMap;
use std::path::Path;
use syn::parse::Parser;
use syn::visit::Visit;
use syn::{punctuated::Punctuated, Attribute, Ident, Item, Meta, Token};

pub type Tests = BTreeMap<String, Vec<Conditions>>;

#[derive(Clone)]
pub struct Conditions {
    // The driver has a Windows memory test and a non-Windows error-contract
    // test under one name. Both bodies must be valid, with no platform omitted
    // or covered twice. Unknown cfg predicates never earn coverage.
    platforms: u8,
    restriction: Option<String>,
}

impl Default for Conditions {
    fn default() -> Self {
        Self {
            platforms: 3,
            restriction: None,
        }
    }
}

impl Conditions {
    fn with_attrs(&self, attrs: &[Attribute]) -> Self {
        let mut conditions = self.clone();
        for attr in attrs {
            if attr.path().is_ident("cfg") {
                let mask = match attr.parse_args::<Meta>() {
                    Ok(Meta::Path(path)) if path.is_ident("test") => Some(3),
                    Ok(Meta::Path(path)) if path.is_ident("windows") => Some(1),
                    Ok(Meta::List(list))
                        if list.path.is_ident("not")
                            && syn::parse2::<Ident>(list.tokens.clone())
                                .is_ok_and(|id| id == "windows") =>
                    {
                        Some(2)
                    }
                    _ => None,
                };
                if let Some(mask) = mask {
                    conditions.platforms &= mask;
                    continue;
                }
            }
            conditions.restriction = conditions
                .restriction
                .or_else(|| restriction(std::slice::from_ref(attr)));
        }
        conditions
    }
}

// Only this verified expansion may supply tests without an ItemFn. Compare
// tokens, so comments/formatting cannot forge or invalidate its #[test] body.
const SIMPLE_TESTS: &str = r#"
macro_rules! simple_tests {
    ($($name:ident),* $(,)?) => {
        $(#[test] fn $name() { Harness::new().case(stringify!($name)); })*
    };
}
"#;

fn restriction(attrs: &[Attribute]) -> Option<String> {
    attrs.iter().find_map(|attr| {
        if attr.path().is_ident("ignore") || attr.path().is_ident("cfg_attr") {
            return Some("ignored or conditional test".into());
        }
        if attr.path().is_ident("cfg") {
            let only_test = matches!(&attr.meta, Meta::List(list)
                if syn::parse2::<Ident>(list.tokens.clone()).is_ok_and(|id| id == "test"));
            if !only_test {
                return Some("conditional test (only cfg(test) is allowed)".into());
            }
        }
        None
    })
}

#[derive(Default)]
struct Body {
    restriction: Option<String>,
}

impl Body {
    // Non-expression macro syntax is outside the supported dialect. Inspect
    // its token groups conservatively; quoted/byte/character literals remain
    // opaque, so strings containing Rust or Lua code do not trigger the guard.
    fn tokens(&mut self, tokens: TokenStream) {
        let mut tokens = tokens.into_iter().peekable();
        while let Some(token) = tokens.next() {
            match token {
                TokenTree::Group(group) => self.tokens(group.stream()),
                TokenTree::Ident(id) if id == "return" => {
                    self.restriction = Some("early return can skip a test".into());
                }
                TokenTree::Ident(id) if id == "cfg" || id == "cfg_attr" => match tokens.peek() {
                    Some(TokenTree::Punct(p)) if p.as_char() == '!' => {
                        self.restriction = Some("runtime cfg can skip a test".into());
                    }
                    Some(TokenTree::Group(_)) => {
                        self.restriction = Some("conditional attribute in macro arguments".into());
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }
}

impl<'ast> Visit<'ast> for Body {
    fn visit_attribute(&mut self, attr: &'ast Attribute) {
        self.restriction = self
            .restriction
            .take()
            .or_else(|| restriction(std::slice::from_ref(attr)));
    }

    fn visit_expr_return(&mut self, _: &'ast syn::ExprReturn) {
        self.restriction = Some("early return can skip a test".into());
    }

    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        if mac.path.segments.last().is_some_and(|s| s.ident == "cfg") {
            self.restriction = Some("runtime cfg can skip a test".into());
            return;
        }
        if let Ok(args) =
            Punctuated::<syn::Expr, Token![,]>::parse_terminated.parse2(mac.tokens.clone())
        {
            for arg in &args {
                self.visit_expr(arg);
            }
        } else {
            self.tokens(mac.tokens.clone());
        }
    }

    // A fixture callback or local helper returns to its caller, not the test.
    fn visit_expr_closure(&mut self, _: &'ast syn::ExprClosure) {}
    fn visit_item(&mut self, _: &'ast Item) {}
}

fn insert(tests: &mut Tests, name: String, conditions: Conditions) {
    tests.entry(name).or_default().push(conditions);
}

fn module_path(module: &syn::ItemMod) -> Result<Option<String>, String> {
    let mut paths = module.attrs.iter().filter(|a| a.path().is_ident("path"));
    let Some(attr) = paths.next() else {
        return Ok(None);
    };
    if paths.next().is_some() {
        return Err("duplicate module path".into());
    }
    if let Meta::NameValue(value) = &attr.meta {
        if let syn::Expr::Lit(expr) = &value.value {
            if let syn::Lit::Str(path) = &expr.lit {
                return Ok(Some(path.value()));
            }
        }
    }
    Err("invalid module path".into())
}

fn reject_macro_scopes(attrs: &[Attribute], prefix: &str) -> Result<(), String> {
    if attrs
        .iter()
        .any(|attr| attr.path().is_ident("macro_use") || attr.path().is_ident("macro_export"))
    {
        return Err(format!("{prefix}: unsupported macro import/export scope"));
    }
    Ok(())
}

// Follow the module graph from a Cargo test target, not every .rs file on disk.
// In rustc 1.95, #[path] files own their parent like mod.rs; implicit foo.rs
// modules own foo/. Inline #[path] overrides supply a directory, not a file.
// https://github.com/rust-lang/rust/blob/1.95.0/compiler/rustc_expand/src/module.rs
fn file(
    path: &Path,
    prefix: &str,
    owns_directory: bool,
    inherited: Conditions,
    macro_verified: bool,
    stack: &mut Vec<std::path::PathBuf>,
    tests: &mut Tests,
) -> Result<(), String> {
    let path = path
        .canonicalize()
        .map_err(|e| format!("{}: {e}", path.display()))?;
    if stack.contains(&path) {
        return Err(format!("cyclic module path: {}", path.display()));
    }
    let source = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    let parsed = syn::parse_file(&source).map_err(|e| format!("{}: {e}", path.display()))?;
    reject_macro_scopes(&parsed.attrs, prefix)?;
    let parent = path.parent().unwrap();
    let directory = if owns_directory || path.file_name().unwrap() == "mod.rs" {
        parent.to_path_buf()
    } else {
        parent.join(path.file_stem().unwrap())
    };
    stack.push(path.clone());
    let result = walk(
        &parsed.items,
        prefix,
        Some((&directory, parent)),
        inherited.with_attrs(&parsed.attrs),
        macro_verified,
        stack,
        tests,
    );
    stack.pop();
    result
}

fn walk(
    items: &[Item],
    prefix: &str,
    directories: Option<(&Path, &Path)>,
    inherited: Conditions,
    mut macro_verified: bool,
    stack: &mut Vec<std::path::PathBuf>,
    tests: &mut Tests,
) -> Result<(), String> {
    for item in items {
        let attrs = match item {
            Item::Mod(module) => module.attrs.as_slice(),
            Item::Macro(mac) => mac.attrs.as_slice(),
            Item::ExternCrate(import) => import.attrs.as_slice(),
            _ => &[],
        };
        // The migrated suites have no macro imports/exports. Refuse these
        // rather than retaining a verified bit after a child changes scope.
        reject_macro_scopes(attrs, prefix)?;
        match item {
            Item::Fn(function) if function.attrs.iter().any(|a| a.path().is_ident("test")) => {
                let mut body = Body::default();
                body.visit_block(&function.block);
                let mut conditions = inherited.with_attrs(&function.attrs);
                conditions.restriction = conditions
                    .restriction
                    .or_else(|| {
                        function
                            .block
                            .stmts
                            .is_empty()
                            .then(|| "empty test body".into())
                    })
                    .or(body.restriction);
                insert(
                    tests,
                    format!("{prefix}::{}", function.sig.ident),
                    conditions,
                );
            }
            Item::Mod(module) => {
                let nested = format!("{prefix}::{}", module.ident);
                let conditions = inherited.with_attrs(&module.attrs);
                let override_path = module_path(module)?;
                if let Some((_, items)) = &module.content {
                    let directory = directories.map(|(dir, path_dir)| {
                        if let Some(path) = &override_path {
                            path_dir.join(path)
                        } else {
                            dir.join(module.ident.to_string())
                        }
                    });
                    walk(
                        items,
                        &nested,
                        directory.as_deref().map(|dir| (dir, dir)),
                        conditions,
                        macro_verified,
                        stack,
                        tests,
                    )?;
                } else if let Some((directory, path_directory)) = directories {
                    let owns_directory = override_path.is_some();
                    let path = if let Some(path) = override_path {
                        path_directory.join(path)
                    } else {
                        let flat = directory.join(format!("{}.rs", module.ident));
                        let folder = directory.join(module.ident.to_string()).join("mod.rs");
                        if flat.is_file() && folder.is_file() {
                            return Err(format!("ambiguous module: {nested}"));
                        }
                        if flat.is_file() {
                            flat
                        } else {
                            folder
                        }
                    };
                    file(
                        &path,
                        &nested,
                        owns_directory,
                        conditions,
                        macro_verified,
                        stack,
                        tests,
                    )?;
                }
            }
            Item::Macro(mac)
                if mac.mac.path.is_ident("macro_rules")
                    && mac.ident.as_ref().is_some_and(|i| i == "simple_tests") =>
            {
                let expected: syn::ItemMacro = syn::parse_str(SIMPLE_TESTS).unwrap();
                let conditions = inherited.with_attrs(&mac.attrs);
                macro_verified = conditions.restriction.is_none()
                    && conditions.platforms == 3
                    && mac.mac.tokens.to_string() == expected.mac.tokens.to_string();
            }
            Item::Macro(mac) if mac.mac.path.is_ident("simple_tests") => {
                let names = Punctuated::<Ident, Token![,]>::parse_terminated
                    .parse2(mac.mac.tokens.clone())
                    .map_err(|e| format!("{prefix}: invalid simple_tests invocation: {e}"))?;
                if names.is_empty() {
                    return Err(format!("{prefix}: empty simple_tests invocation"));
                }
                let mut conditions = inherited.with_attrs(&mac.attrs);
                conditions.restriction = conditions.restriction.or_else(|| {
                    (!macro_verified).then(|| "unverified simple_tests definition".into())
                });
                for name in names {
                    insert(tests, format!("{prefix}::{name}"), conditions.clone());
                }
            }
            Item::Macro(mac)
                if mac
                    .mac
                    .path
                    .segments
                    .last()
                    .is_some_and(|s| s.ident == "simple_tests") =>
            {
                return Err(format!(
                    "{prefix}: unsupported macro qualified simple_tests invocation"
                ));
            }
            _ => {}
        }
    }
    Ok(())
}

pub fn resolve(path: &Path, target: &str) -> Result<Tests, String> {
    let mut tests = Tests::new();
    file(
        path,
        target,
        true,
        Conditions::default(),
        false,
        &mut Vec::new(),
        &mut tests,
    )?;
    Ok(tests)
}

pub fn parse(source: &str, target: &str) -> Result<Tests, String> {
    let parsed = syn::parse_file(source).map_err(|e| e.to_string())?;
    reject_macro_scopes(&parsed.attrs, target)?;
    let mut tests = Tests::new();
    walk(
        &parsed.items,
        target,
        None,
        Conditions::default().with_attrs(&parsed.attrs),
        false,
        &mut Vec::new(),
        &mut tests,
    )?;
    Ok(tests)
}

pub fn require(tests: &Tests, name: &str) -> Result<(), String> {
    let definitions = tests
        .get(name)
        .ok_or_else(|| format!("missing #[test] {name}"))?;
    let mut platforms = 0;
    for definition in definitions {
        if let Some(reason) = &definition.restriction {
            return Err(format!("{name}: {reason}"));
        }
        if platforms & definition.platforms != 0 {
            return Err(format!("{name}: duplicate Rust test definition"));
        }
        if definition.platforms == 0 {
            return Err(format!("{name}: unreachable conditional test"));
        }
        platforms |= definition.platforms;
    }
    if platforms != 3 {
        return Err(format!("{name}: conditional test omits a platform"));
    }
    Ok(())
}
