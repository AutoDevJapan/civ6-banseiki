//! Offline packaging for the five normal bootstrap software artifacts.
//! This produces build data only; it never loads a DLL or opens a game.
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

const TARGET: &str = "x86_64-pc-windows-msvc";
const LIMIT: u64 = 128 * 1024 * 1024;
const PROFILE_DEFAULT_KEYS: &[&str] = &[
    "CARGO_PROFILE_RELEASE_PANIC",
    "CARGO_PROFILE_RELEASE_LTO",
    "CARGO_PROFILE_RELEASE_CODEGEN_UNITS",
    "CARGO_PROFILE_RELEASE_OPT_LEVEL",
    "CARGO_PROFILE_RELEASE_DEBUG",
    "CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS",
    "CARGO_PROFILE_RELEASE_OVERFLOW_CHECKS",
    "CARGO_PROFILE_RELEASE_RPATH",
    "CARGO_PROFILE_RELEASE_SPLIT_DEBUGINFO",
    "CARGO_PROFILE_RELEASE_STRIP",
    "CARGO_PROFILE_RELEASE_INCREMENTAL",
    "CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_CODEGEN_UNITS",
    "CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_OPT_LEVEL",
    "CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_DEBUG",
];
const BOOTSTRAP: &[(&str, bool)] = &[
    ("RustBridgeBootstrapPrepare", true),
    ("RustBridgeBootstrapStopObservation", true),
    ("BOOTSTRAP_TRACE_POINTER", false),
    ("BOOTSTRAP_PREPARE_DIAGNOSTIC_POINTER", false),
    ("BOOTSTRAP_PREPARE_DIAGNOSTIC_SIZE", false),
];
const RUNTIME: &[(&str, bool)] = &[
    ("BridgeRuntimeInstall", true),
    ("BridgeRuntimeClose", true),
    ("BridgeRuntimeRebuild", true),
    ("BridgeRuntimeStopObservation", true),
    ("BRIDGE_RUNTIME_PROPERTY_LOG", false),
    ("BRIDGE_RUNTIME_NATIVE_FRAME_TRACE", false),
    ("BRIDGE_RUNTIME_NATIVE_CENSUS", false),
    ("BRIDGE_RUNTIME_NATIVE_CENSUS_SIZE", false),
];

#[derive(Clone, Copy)]
struct Role {
    name: &'static str,
    package: &'static str,
    manifest: &'static str,
    target: &'static str,
    phase: &'static str,
    relative: &'static str,
    dll: bool,
    exports: &'static [(&'static str, bool)],
}
const ROLES: [Role; 5] = [
    Role {
        name: "civ6-autotest.exe",
        package: "civ6-autotest-rs",
        manifest: "tools/civ6-autotest-rs/Cargo.toml",
        target: "civ6-autotest",
        phase: "bins",
        relative: "tools/target/release/civ6-autotest.exe",
        dll: false,
        exports: &[],
    },
    Role {
        name: "civ6-startup-diagnostics.exe",
        package: "civ6-autotest-rs",
        manifest: "tools/civ6-autotest-rs/Cargo.toml",
        target: "civ6-startup-diagnostics",
        phase: "bins",
        relative: "tools/target/release/civ6-startup-diagnostics.exe",
        dll: false,
        exports: &[],
    },
    Role {
        name: "civ6-bootstrap-scene.exe",
        package: "civ6-autotest-rs",
        manifest: "tools/civ6-autotest-rs/Cargo.toml",
        target: "civ6-bootstrap-scene",
        phase: "bins",
        relative: "tools/target/release/civ6-bootstrap-scene.exe",
        dll: false,
        exports: &[],
    },
    Role {
        name: "civ6_native_bridge_rs.dll",
        package: "civ6-native-bridge-rs",
        manifest: "tools/civ6-native-bridge-rs/Cargo.toml",
        target: "civ6_native_bridge_rs",
        phase: "dlls",
        relative: "tools/target/x86_64-pc-windows-msvc/release/civ6_native_bridge_rs.dll",
        dll: true,
        exports: RUNTIME,
    },
    Role {
        name: "civ6_bootstrap_rs.dll",
        package: "civ6-bootstrap-rs",
        manifest: "tools/civ6-bootstrap-rs/Cargo.toml",
        target: "civ6_bootstrap_rs",
        phase: "dlls",
        relative: "tools/target/x86_64-pc-windows-msvc/release/civ6_bootstrap_rs.dll",
        dll: true,
        exports: BOOTSTRAP,
    },
];

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn text<'a>(v: &'a Value, key: &str) -> Result<&'a str, String> {
    v[key].as_str().ok_or_else(|| format!("{key} string required"))
}
fn hex(value: &str, length: usize) -> bool {
    value.len() == length && value.bytes().all(|b| b.is_ascii_hexdigit())
}
fn read(path: &Path) -> Result<Vec<u8>, String> {
    read_input(path, false)
}
fn read_input(path: &Path, allow_empty: bool) -> Result<Vec<u8>, String> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(1);
    }
    let mut file = options.open(path).map_err(|e| format!("input open: {e}"))?;
    let size = file.metadata().map_err(|e| e.to_string())?.len();
    if (!allow_empty && size == 0) || size > LIMIT {
        return Err("input size is empty or exceeds the artifact bound".into());
    }
    let mut bytes = Vec::with_capacity(size as usize);
    (&mut file)
        .take(LIMIT + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 != size {
        return Err("input length changed while reading".into());
    }
    Ok(bytes)
}
fn at(bytes: &[u8], offset: usize, count: usize) -> Result<&[u8], String> {
    bytes
        .get(offset..offset.checked_add(count).ok_or("PE offset overflow")?)
        .ok_or_else(|| "truncated PE".into())
}
fn u16_at(b: &[u8], p: usize) -> Result<u16, String> {
    Ok(u16::from_le_bytes(at(b, p, 2)?.try_into().unwrap()))
}
fn u32_at(b: &[u8], p: usize) -> Result<u32, String> {
    Ok(u32::from_le_bytes(at(b, p, 4)?.try_into().unwrap()))
}

fn pe_exports(bytes: &[u8], dll: bool, required: &[(&str, bool)]) -> Result<Value, String> {
    if at(bytes, 0, 2)? != b"MZ" {
        return Err("not an MZ image".into());
    }
    let pe = u32_at(bytes, 0x3c)? as usize;
    if at(bytes, pe, 4)? != b"PE\0\0" {
        return Err("not a PE image".into());
    }
    let coff = pe.checked_add(4).ok_or("PE offset overflow")?;
    if u16_at(bytes, coff)? != 0x8664 {
        return Err("product artifact is not Windows x64".into());
    }
    let characteristics = u16_at(bytes, coff + 18)?;
    if characteristics & 0x0002 == 0 {
        return Err("PE executable image characteristic is absent".into());
    }
    if (characteristics & 0x2000 != 0) != dll {
        return Err("PE executable/DLL kind differs from Cargo role".into());
    }
    let count = u16_at(bytes, coff + 2)? as usize;
    let optional = coff + 20;
    let optional_size = u16_at(bytes, coff + 16)? as usize;
    if count == 0 || count > 96 || optional_size < 120 || u16_at(bytes, optional)? != 0x20b {
        return Err("invalid PE32+ image".into());
    }
    let table = optional
        .checked_add(optional_size)
        .ok_or("PE section overflow")?;
    let mut sections = Vec::new();
    for i in 0..count {
        let s = table
            .checked_add(i.checked_mul(40).ok_or("PE section overflow")?)
            .ok_or("PE section overflow")?;
        let size = u32_at(bytes, s + 16)? as usize;
        let offset = u32_at(bytes, s + 20)? as usize;
        at(bytes, offset, size)?;
        sections.push((
            u32_at(bytes, s + 12)?, u32_at(bytes, s + 8)? as usize,
            size, offset, u32_at(bytes, s + 36)?,
        ));
    }
    let map = |rva: u32, length: usize| -> Result<(usize, bool), String> {
        sections
            .iter()
            .find_map(|(base, virtual_size, size, offset, flags)| {
                let relative = rva.checked_sub(*base)? as usize;
                let end = relative.checked_add(length)?;
                let mapped = if *virtual_size == 0 { *size } else { *virtual_size };
                (end <= *size && end <= mapped)
                    .then(|| offset.checked_add(relative).map(|p| (p, flags & 0x2000_0000 != 0)))
                    .flatten()
            })
            .ok_or_else(|| "required PE RVA is not backed by image bytes".into())
    };
    if required.is_empty() {
        return Ok(json!([]));
    }
    if u32_at(bytes, optional + 108)? == 0 {
        return Err("DLL export directory absent".into());
    }
    let export_rva = u32_at(bytes, optional + 112)?;
    let export_size = u32_at(bytes, optional + 116)?;
    let export_end = export_rva
        .checked_add(export_size)
        .ok_or("PE export overflow")?;
    if export_size < 40 {
        return Err("invalid PE export directory length".into());
    }
    let directory = map(export_rva, 40)?.0;
    let functions = u32_at(bytes, directory + 20)? as usize;
    let names = u32_at(bytes, directory + 24)? as usize;
    if functions == 0 || functions > 65536 || names > functions {
        return Err("invalid PE export counts".into());
    }
    let function_table = map(u32_at(bytes, directory + 28)?, functions * 4)?.0;
    let name_table = map(u32_at(bytes, directory + 32)?, names * 4)?.0;
    let ordinal_table = map(u32_at(bytes, directory + 36)?, names * 2)?.0;
    let mut found = BTreeMap::new();
    for i in 0..names {
        let name_rva = u32_at(bytes, name_table + i * 4)?;
        let string = map(name_rva, 1)?.0;
        let tail = bytes.get(string..).ok_or("invalid PE name")?;
        let length = tail
            .iter()
            .take(256)
            .position(|b| *b == 0)
            .ok_or("unbounded PE export name")?;
        map(name_rva, length + 1)?;
        let name = std::str::from_utf8(&tail[..length]).map_err(|_| "non-UTF8 PE export")?;
        let ordinal = u16_at(bytes, ordinal_table + i * 2)? as usize;
        if ordinal >= functions || found.contains_key(name) {
            return Err("duplicate or invalid PE export ordinal".into());
        }
        found.insert(name, u32_at(bytes, function_table + ordinal * 4)?);
    }
    let mut exports = Vec::new();
    for (name, function) in required {
        let rva = *found.get(name).ok_or_else(|| format!("required export missing: {name}"))?;
        if rva >= export_rva && rva < export_end {
            return Err("forwarded export cannot satisfy the normal product ABI".into());
        }
        let executable = if *function {
            map(rva, 1)?.1
        } else {
            sections.iter().find_map(|(base, virtual_size, raw_size, _, flags)| {
                let relative = rva.checked_sub(*base)? as usize;
                (relative.checked_add(8)? <= (*virtual_size).max(*raw_size))
                    .then_some(flags & 0x2000_0000 != 0)
            }).ok_or("data export is outside its virtual section")?
        };
        if executable != *function {
            return Err(format!("required export code/data kind differs: {name}"));
        }
        exports.push(json!({"name":name,"kind":if *function {"function"} else {"data"},"rva":rva}));
    }
    Ok(json!(exports))
}

fn phase_argv(phase: &str) -> Vec<&'static str> {
    let mut command = vec![
        "cargo", "+1.95.0", "build", "--locked", "--release",
        "--manifest-path", "tools/Cargo.toml",
    ];
    if phase == "bins" {
        command.extend(["--workspace", "--bins", "--message-format=json-render-diagnostics"]);
    } else {
        command.extend([
            "--package", "civ6-native-bridge-rs", "--package", "civ6-bootstrap-rs",
            "--lib", "--target", TARGET, "--message-format=json-render-diagnostics",
        ]);
    }
    command
}

fn validate_context(context: &Value, completion: &Value, context_sha: &str) -> Result<(), String> {
    if context["schema"] != "civ6-product-build-context/v1"
        || context["repository"] != "AutoDevJapan/civ6-banseiki"
        || context["repository_id"] != 1392152644u64
        || context["target"] != TARGET
        || context["compiler"]["host"] != TARGET
        || context["tracked_clean"] != true
        || !hex(text(context, "source_sha")?, 40)
        || !hex(text(context, "source_tree")?, 40)
        || !hex(text(&context["inputs"], "cargo_lock_sha256")?, 64)
        || !hex(text(&context["compiler"], "rustc_sha256")?, 64)
        || !hex(text(&context["compiler"], "cargo_sha256")?, 64)
        || !hex(text(&context["compiler"], "rustup_manifest_sha256")?, 64)
        || !hex(text(&context["compiler"], "cargo_dispatcher_sha256")?, 64)
        || !hex(text(&context["compiler"], "rustup_dispatcher_sha256")?, 64)
        || !text(&context["compiler"], "rustc_verbose")?.contains("release: 1.95.0")
        || completion["schema"] != "civ6-product-build-completion/v1"
        || completion["context_sha256"] != context_sha
        || completion["source_sha"] != context["source_sha"]
        || completion["source_tree"] != context["source_tree"]
        || completion["inputs"] != context["inputs"]
        || completion["compiler"] != context["compiler"]
        || completion["tracked_clean"] != true
    {
        return Err("build source/compiler/input context differs or is incomplete".into());
    }
    for phase in ["bins", "dlls"] {
        let binding = &completion["commands"][phase]["compiler_environment"];
        let compiler = text(&context["compiler"], "rustc_path")?;
        let environment = binding.as_object().ok_or("actual compiler environment absent")?;
        if compiler.is_empty()
            || environment.len() != 9
            || binding["RUSTC"] != compiler
            || binding["CARGO_BUILD_RUSTC"] != compiler
            || binding["RUSTC_WRAPPER"] != ""
            || binding["RUSTC_WORKSPACE_WRAPPER"] != ""
            || binding["CARGO_ENCODED_RUSTFLAGS"] != ""
            || !environment.contains_key("RUSTFLAGS") || !binding["RUSTFLAGS"].is_null()
            || !environment.contains_key("CARGO_BUILD_RUSTFLAGS")
            || !binding["CARGO_BUILD_RUSTFLAGS"].is_null()
            || !environment.contains_key("CARGO_BUILD_RUSTC_WRAPPER")
            || !binding["CARGO_BUILD_RUSTC_WRAPPER"].is_null()
            || !environment.contains_key("CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER")
            || !binding["CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER"].is_null()
        {
            return Err("actual compiler environment differs from its fixed selection".into());
        }
        let expected = json!(phase_argv(phase));
        if context["commands"][phase] != expected
            || completion["commands"][phase]["argv"] != expected
            || completion["commands"][phase]["exit_code"] != 0
            || completion["commands"][phase]["collection_complete"] != true
            || completion["commands"][phase]["stdout_eof"] != true
            || completion["commands"][phase]["stderr_eof"] != true
            || completion["commands"][phase]["root_exit_observed"] != true
            || completion["commands"][phase]["stop_reason"] != "completed"
            || completion["commands"][phase]["profile_environment"] != context["inputs"]["profile_environment"]
            || !hex(text(&completion["commands"][phase], "stderr_sha256")?, 64)
            || completion["commands"][phase]["native_pid"].as_u64().is_none_or(|n| n == 0)
            || text(&completion["commands"][phase], "native_started_utc")?.is_empty()
        {
            return Err(format!("actual {phase} command/exit does not match pre-build plan"));
        }
    }
    for field in ["configs", "vendors"] {
        let entries = context["inputs"][field]
            .as_array()
            .ok_or_else(|| format!("pre-build {field} facts absent"))?;
        for entry in entries {
            if !hex(text(entry, "sha256")?, 64) {
                return Err("input digest is invalid".into());
            }
        }
    }
    if context["inputs"]["vendors"].as_array().unwrap().is_empty() {
        return Err("vendor source facts absent".into());
    }
    Ok(())
}

// SHA-256 of each ordered UTF-8 "sha256 git-blob-or-- bytes relative-name\\n" row.
fn public_file_inventory(value: &Value) -> Result<(Value, String), String> {
    let entries = value.as_array().ok_or("input file facts absent")?;
    if entries.is_empty() || entries.len() > 200_000 {
        return Err("input file count is outside its bound".into());
    }
    let mut names = BTreeSet::new();
    let mut rows = Vec::new();
    let mut canonical = String::new();
    for entry in entries {
        let name = text(entry, "name")?;
        let sha = text(entry, "sha256")?;
        let bytes = entry["bytes"].as_u64().ok_or("input byte length absent")?;
        if name.is_empty() || name.len() > 4096
            || name.contains('\\') || name.contains(':')
            || name.chars().any(char::is_control)
            || name.split('/').any(|part| part.is_empty() || part == "." || part == "..")
            || bytes > 512 * 1024 * 1024 || !hex(sha, 64)
            || !names.insert(name)
        {
            return Err("input file name/hash/length is invalid or duplicated".into());
        }
        let git_blob = &entry["git_blob_sha1"];
        let git_text = if git_blob.is_null() {
            "-"
        } else {
            let git = git_blob.as_str().ok_or("Git blob digest must be text")?;
            if !hex(git, 40) {
                return Err("Git blob digest invalid".into());
            }
            git
        };
        canonical.push_str(&format!("{sha} {git_text} {bytes} {name}\n"));
        rows.push(json!({"name":name,"bytes":bytes,"sha256":sha,"git_blob_sha1":git_blob}));
    }
    Ok((json!(rows), digest(canonical.as_bytes())))
}

fn public_profile_environment(value: &Value) -> Result<Value, String> {
    let entries = value.as_array().ok_or("profile environment absent")?;
    if entries.len() > 4096 {
        return Err("profile environment exceeds bound".into());
    }
    let mut names = BTreeSet::new();
    let mut rows = Vec::new();
    for entry in entries {
        let name = text(entry, "name")?;
        let present = entry["present"].as_bool().ok_or("profile presence must be boolean")?;
        if !name.starts_with("CARGO_PROFILE_")
            || !name.bytes().all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
            || !names.insert(name)
            || (present && !entry["sha256"].as_str().is_some_and(|s| hex(s, 64)))
            || (!present && !entry["sha256"].is_null())
        {
            return Err("profile environment name/presence/hash invalid".into());
        }
        rows.push(json!({"name":name,"present":present,"sha256":entry["sha256"]}));
    }
    if PROFILE_DEFAULT_KEYS.iter().any(|name| !names.contains(name)) {
        return Err("profile default/absence facts incomplete".into());
    }
    Ok(json!(rows))
}

fn public_inputs(inputs: &Value) -> Result<Value, String> {
    if !hex(text(inputs, "source_inventory_sha256")?, 64)
        || inputs["source_file_count"].as_u64().is_none_or(|n| n == 0)
    {
        return Err("pre-build source inventory is incomplete".into());
    }
    let (source_files, source_digest) = public_file_inventory(&inputs["source_files"])?;
    if source_files.as_array().unwrap().iter().any(|f| f["git_blob_sha1"].is_null())
        || inputs["source_inventory_sha256"] != source_digest
        || inputs["source_file_count"].as_u64()
            != Some(source_files.as_array().unwrap().len() as u64)
    {
        return Err("complete source input list differs from its digest/count".into());
    }
    let mut keys = BTreeSet::new();
    let mut configs = Vec::new();
    for item in inputs["configs"].as_array().ok_or("config inputs absent")? {
        let scope = text(item, "scope")?;
        let name = text(item, "name")?;
        let ancestor = scope.strip_prefix("ancestor-")
            .is_some_and(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()));
        if !(scope == "cargo-home" || ancestor)
            || !matches!(name, "config" | "config.toml")
            || item["bytes"].as_u64().is_none_or(|n| n > LIMIT)
            || !hex(text(item, "sha256")?, 64)
            || !keys.insert(format!("{scope}/{name}"))
        {
            return Err("invalid or duplicate public config facts".into());
        }
        configs.push(json!({"scope":scope,"name":name,"sha256":item["sha256"],"bytes":item["bytes"]}));
    }
    let mut vendors = Vec::new();
    for item in inputs["vendors"].as_array().ok_or("vendor inputs absent")? {
        let name = text(item, "name")?;
        let version = text(item, "version")?;
        if name.is_empty() || version.is_empty()
            || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
            || !version.bytes().all(|b| b.is_ascii_alphanumeric() || b".-+".contains(&b))
            || item["source_kind"] != "crates.io"
            || !hex(text(item, "sha256")?, 64)
            || !hex(text(item, "manifest_sha256")?, 64)
            || !keys.insert(format!("vendor/{name}@{version}"))
        {
            return Err("invalid or duplicate public vendor facts".into());
        }
        let (files, inventory_digest) = public_file_inventory(&item["files"])?;
        let storage = text(item, "storage_kind")?;
        let checksum = &item["checksum_metadata_sha256"];
        if item["sha256"] != inventory_digest
            || !matches!(storage, "registry-extracted" | "directory")
            || !(checksum.is_null() && storage == "registry-extracted"
                || checksum.as_str().is_some_and(|s| hex(s, 64)))
        {
            return Err("dependency storage/checksum/input facts differ".into());
        }
        let facts = files.as_array().unwrap();
        if !facts.iter().any(|f| f["name"] == "Cargo.toml"
            && f["sha256"] == item["manifest_sha256"])
            || !(checksum.is_null() && !facts.iter().any(|f| f["name"] == ".cargo-checksum.json")
                || facts.iter().any(|f| f["name"] == ".cargo-checksum.json"
                    && f["sha256"] == *checksum))
        {
            return Err("dependency manifest/checksum facts are not in the complete inventory".into());
        }
        vendors.push(json!({
            "name":name,"version":version,"source_kind":"crates.io","storage_kind":storage,
            "sha256":item["sha256"],"manifest_sha256":item["manifest_sha256"],
            "checksum_metadata_sha256":checksum,"files":files,
        }));
    }
    if vendors.is_empty() {
        return Err("vendor source facts absent".into());
    }
    let mut environment = Vec::new();
    for item in inputs["environment"].as_array().ok_or("build environment facts absent")? {
        let name = text(item, "name")?;
        if !matches!(
            name,
            "CARGO_INCREMENTAL" | "CARGO_PROFILE_DEV_DEBUG" | "CARGO_PROFILE_TEST_DEBUG"
        ) || !hex(text(item, "sha256")?, 64)
            || !keys.insert(format!("environment/{name}"))
        {
            return Err("unrecognized public environment fact".into());
        }
        environment.push(json!({"name":name,"sha256":item["sha256"]}));
    }
    if environment.len() != 3 {
        return Err("the three fixed build environment facts are required".into());
    }
    let profile_environment = public_profile_environment(&inputs["profile_environment"])?;
    Ok(json!({
        "cargo_lock_sha256":inputs["cargo_lock_sha256"],
        "source_inventory_sha256":inputs["source_inventory_sha256"],
        "source_file_count":inputs["source_file_count"],
        "inventory_format":"sha256-lines-v2","source_files":source_files,
        "configs":configs,"vendors":vendors,"environment":environment,
        "profile_environment":profile_environment,
    }))
}

fn public_compiler(context: &Value) -> Result<Value, String> {
    let compiler = &context["compiler"];
    let verbose = text(compiler, "rustc_verbose")?;
    if compiler["toolchain"] != "1.95.0"
        || !text(compiler, "cargo_version")?.starts_with("cargo 1.95.0 ")
        || !text(compiler, "cargo_version")?.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b" .()-".contains(&b))
        || verbose.lines().any(|line| {
            line.contains('/') || line.contains('\\')
                || ![
                    "rustc ", "binary: rustc", "commit-hash:", "commit-date:",
                    "host:", "release:", "LLVM version:",
                ].iter().any(|prefix| line.starts_with(prefix))
        })
    {
        return Err("compiler facts contain unsupported or private data".into());
    }
    Ok(json!({
        "toolchain":"1.95.0","host":TARGET,"rustc_verbose":verbose,
        "cargo_version":compiler["cargo_version"],"rustc_sha256":compiler["rustc_sha256"],
        "cargo_sha256":compiler["cargo_sha256"],
        "rustup_manifest_sha256":compiler["rustup_manifest_sha256"],
        "cargo_dispatcher_sha256":compiler["cargo_dispatcher_sha256"],
        "rustup_dispatcher_sha256":compiler["rustup_dispatcher_sha256"],
    }))
}

fn public_producer(context: &Value) -> Result<Value, String> {
    let producer = &context["producer"];
    if producer["run_id"].as_u64().is_none_or(|n| n == 0)
        || producer["run_attempt"].as_u64().is_none_or(|n| n == 0)
        || producer["job"] != "test"
        || producer["runner_os"] != "Windows"
        || producer["runner_arch"] != "X64"
        || !hex(text(producer, "workflow_blob")?, 40)
    {
        return Err("normal CI producer origin is incomplete".into());
    }
    Ok(json!({
        "run_id":producer["run_id"],"run_attempt":producer["run_attempt"],
        "job":"test","runner_os":"Windows","runner_arch":"X64",
        "workflow_blob":producer["workflow_blob"],
    }))
}

fn cargo_records(bytes: &[u8]) -> Result<Vec<(Value, String)>, String> {
    let text = std::str::from_utf8(bytes).map_err(|_| "Cargo output is not UTF8")?;
    let mut artifacts = Vec::new();
    let mut finished = 0;
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        // Other build tools may write ordinary stdout; keep it in the raw digest.
        // A malformed JSON message is still rejected.
        if !line.starts_with('{') {
            continue;
        }
        let record: Value = serde_json::from_str(line).map_err(|_| "invalid Cargo JSON record")?;
        if record["reason"] == "build-finished" {
            if record["success"] != true {
                return Err("Cargo build did not finish successfully".into());
            }
            finished += 1;
        } else if record["reason"] == "compiler-artifact" {
            artifacts.push((record, digest(line.as_bytes())));
        }
    }
    if finished != 1 {
        return Err("one actual Cargo build-finished record is required".into());
    }
    Ok(artifacts)
}

fn selected_record(
    records: &[(Value, String)],
    packages: &[Value],
    root: &Path,
    role: Role,
) -> Result<Value, String> {
    let expected_manifest = root
        .join(role.manifest)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let packages: Vec<_> = packages
        .iter()
        .filter(|p| p["name"] == role.package)
        .collect();
    if packages.len() != 1 {
        return Err("required workspace package is missing or duplicated".into());
    }
    let package = packages[0];
    let package_id = text(package, "id")?;
    if package_id.is_empty()
        || Path::new(text(package, "manifest_path")?)
            .canonicalize()
            .map_err(|e| e.to_string())? != expected_manifest
    {
        return Err("workspace package identity/manifest differs from its role".into());
    }
    let matches: Vec<_> = records
        .iter()
        .filter(|(r, _)| r["package_id"] == package["id"] && r["target"]["name"] == role.target)
        .collect();
    if matches.len() != 1 {
        return Err(format!("one actual compiler-artifact is required for {}", role.name));
    }
    let (record, raw_sha) = matches[0];
    let manifest = Path::new(text(record, "manifest_path")?)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let kind = if role.dll { "cdylib" } else { "bin" };
    let version = text(package, "version")?;
    let valid_kinds = |values: &Value| {
        values.as_array().is_some_and(|v| {
            !v.is_empty() && v.len() <= 2
                && v.iter().all(|k| if role.dll { k == "cdylib" || k == "rlib" } else { k == "bin" })
        })
    };
    if manifest != expected_manifest
        || version.is_empty()
        || !version.bytes().all(|b| b.is_ascii_alphanumeric() || b".-+".contains(&b))
        || record["fresh"].as_bool().is_none()
        || !valid_kinds(&record["target"]["kind"])
        || !valid_kinds(&record["target"]["crate_types"])
        || record["profile"]["test"] != false
        || !record["target"]["kind"]
            .as_array()
            .is_some_and(|v| v.iter().any(|k| k == kind))
        || !record["target"]["crate_types"]
            .as_array()
            .is_some_and(|v| v.iter().any(|k| k == kind))
    {
        return Err("Cargo package/target/kind is not the required release software".into());
    }
    let expected = root.join(role.relative).canonicalize().map_err(|e| e.to_string())?;
    let names = record["filenames"].as_array().ok_or("Cargo filenames absent")?;
    let mut matching = 0;
    for name in names {
        let path = Path::new(name.as_str().ok_or("Cargo filename is not text")?);
        if path.file_name() == Some(std::ffi::OsStr::new(role.name)) {
            if path.canonicalize().map_err(|e| e.to_string())? != expected {
                return Err("Cargo output path differs from the source-bound release target".into());
            }
            matching += 1;
        }
    }
    if matching != 1 || !expected.starts_with(root) {
        return Err("required Cargo output is absent or outside this source".into());
    }
    let profile = &record["profile"];
    let opt_level = text(profile, "opt_level")?;
    let debug = &profile["debuginfo"];
    if !matches!(opt_level, "0" | "1" | "2" | "3" | "s" | "z")
        || !(debug.is_null()
            || debug.as_u64().is_some_and(|n| n <= 2)
            || debug == "line-directives-only"
            || debug == "line-tables-only")
        || profile["debug_assertions"].as_bool().is_none()
        || profile["overflow_checks"].as_bool().is_none()
    {
        return Err("actual Cargo profile is incomplete".into());
    }
    let mut features = BTreeSet::new();
    for value in record["features"].as_array().ok_or("Cargo features absent")? {
        let feature = value.as_str().ok_or("Cargo feature must be text")?;
        if feature.is_empty()
            || !feature.bytes().all(|b| b.is_ascii_alphanumeric() || b"-_.+".contains(&b))
            || !features.insert(feature)
        {
            return Err("Cargo feature facts are invalid or duplicated".into());
        }
    }
    if role.dll {
        if !record["executable"].is_null() {
            return Err("a DLL artifact cannot have a binary executable role".into());
        }
    } else if Path::new(text(record, "executable")?)
        .canonicalize()
        .map_err(|e| e.to_string())? != expected
    {
        return Err("actual Cargo executable differs from its selected filename".into());
    }
    Ok(json!({
        "package":{"name":role.package,"version":version,"manifest":role.manifest,
            "id_sha256":digest(package_id.as_bytes())},
        "target":{"name":role.target,"kind":record["target"]["kind"],"crate_types":record["target"]["crate_types"]},
        "filename":role.relative,
        "cargo_record_sha256":raw_sha,
        "profile":{
            "opt_level":opt_level,"debuginfo":debug,
            "debug_assertions":profile["debug_assertions"],
            "overflow_checks":profile["overflow_checks"],"test":false,
        },
        "features":features,
        "fresh":record["fresh"],
    }))
}

fn create(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .read(true)
        .create_new(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    file.write_all(bytes).map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    drop(file);
    if read(path)? != bytes {
        return Err("new artifact copy differs after writing".into());
    }
    Ok(())
}

/// Package only bytes from this completed release build. No input is executed.
pub fn write_bundle(
    context_path: &Path,
    bins_path: &Path,
    dlls_path: &Path,
    completion_path: &Path,
    output: &Path,
) -> Result<(), String> {
    let context_bytes = read(context_path)?;
    let context: Value = serde_json::from_slice(&context_bytes).map_err(|e| e.to_string())?;
    let completion_bytes = read(completion_path)?;
    let completion: Value =
        serde_json::from_slice(&completion_bytes).map_err(|e| e.to_string())?;
    validate_context(&context, &completion, &digest(&context_bytes))?;
    let inputs = public_inputs(&context["inputs"])?;
    let after_inputs = public_inputs(&completion["inputs"])?;
    let compiler = public_compiler(&context)?;
    let producer = public_producer(&context)?;
    let bins_bytes = read(bins_path)?;
    let dlls_bytes = read(dlls_path)?;
    if completion["commands"]["bins"]["cargo_json_sha256"] != digest(&bins_bytes)
        || completion["commands"]["dlls"]["cargo_json_sha256"] != digest(&dlls_bytes)
    {
        return Err("actual Cargo output bytes differ from completion".into());
    }
    for (path, phase) in [(bins_path, "bins"), (dlls_path, "dlls")] {
        let mut stderr_path = path.as_os_str().to_os_string();
        stderr_path.push(".stderr.raw");
        let stderr = read_input(Path::new(&stderr_path), true)?;
        if completion["commands"][phase]["stderr_sha256"] != digest(&stderr) {
            return Err("original stderr bytes differ from completed capture".into());
        }
    }
    let bins = cargo_records(&bins_bytes)?;
    let dlls = cargo_records(&dlls_bytes)?;
    let root = PathBuf::from(text(&context, "source_root")?)
        .canonicalize()
        .map_err(|e| e.to_string())?;
    let packages = context["workspace_packages"].as_array().ok_or("workspace packages absent")?;
    let mut payloads = Vec::new();
    let mut rows = Vec::new();
    let mut records = Vec::new();
    for role in ROLES {
        let selected = selected_record(
            if role.phase == "bins" { &bins } else { &dlls },
            packages,
            &root,
            role,
        )?;
        let bytes = read(&root.join(role.relative))?;
        let exports = pe_exports(&bytes, role.dll, role.exports)?;
        rows.push(json!({
            "name":role.name,"bytes":bytes.len(),"sha256":digest(&bytes),
            "target":TARGET,"kind":if role.dll {"cdylib"} else {"bin"},"exports":exports,
            "cargo_record_sha256":selected["cargo_record_sha256"],
        }));
        records.push(selected);
        payloads.push((role.name, bytes));
    }
    let cargo_metadata = serde_json::to_vec_pretty(
        &json!({
            "schema":"civ6-selected-cargo-artifacts/v1","records":records,
            "inputs_before":inputs,"inputs_after":after_inputs,
        }),
    ).map_err(|e| e.to_string())?;
    let manifest = json!({
        "schema":"civ6-windows-product-artifacts/v1",
        "repository":"AutoDevJapan/civ6-banseiki","repository_id":1392152644u64,
        "source_sha":context["source_sha"],"source_tree":context["source_tree"],
        "target":TARGET,"compiler":compiler,"inputs":inputs,"producer":producer,
        "commands":{
            "bins":{"argv":phase_argv("bins"),"exit_code":0,"cargo_json_sha256":digest(&bins_bytes),"stderr_sha256":completion["commands"]["bins"]["stderr_sha256"]},
            "dlls":{"argv":phase_argv("dlls"),"exit_code":0,"cargo_json_sha256":digest(&dlls_bytes),"stderr_sha256":completion["commands"]["dlls"]["stderr_sha256"]},
        },
        "context_sha256":digest(&context_bytes),
        "completion_sha256":digest(&completion_bytes),
        "source_declared_prepare_version":civ6_bootstrap_rs::VERSION,
        "cargo_artifacts_sha256":digest(&cargo_metadata),
        "files":rows,
    });
    // Absolute roots/package IDs stay in private CI inputs, never in the bundle.
    fs::create_dir(output).map_err(|e| e.to_string())?;
    for (name, bytes) in payloads {
        create(&output.join(name), &bytes)?;
    }
    create(
        &output.join("manifest.json"),
        &serde_json::to_vec_pretty(&manifest).map_err(|e| e.to_string())?,
    )?;
    create(
        &output.join("cargo-artifacts.json"),
        &cargo_metadata,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn put16(b: &mut [u8], p: usize, n: u16) {
        b[p..p + 2].copy_from_slice(&n.to_le_bytes());
    }
    fn put32(b: &mut [u8], p: usize, n: u32) {
        b[p..p + 4].copy_from_slice(&n.to_le_bytes());
    }
    fn image(dll: bool, exports: &[(&str, bool)]) -> Vec<u8> {
        let mut b = vec![0; 0x1400];
        b[..2].copy_from_slice(b"MZ");
        put32(&mut b, 0x3c, 0x80);
        b[0x80..0x84].copy_from_slice(b"PE\0\0");
        put16(&mut b, 0x84, 0x8664);
        put16(&mut b, 0x86, 2);
        put16(&mut b, 0x94, 0xf0);
        put16(&mut b, 0x96, if dll { 0x2022 } else { 0x22 });
        put16(&mut b, 0x98, 0x20b);
        put32(&mut b, 0x98 + 108, 16);
        put32(&mut b, 0x98 + 112, 0x2000);
        put32(&mut b, 0x98 + 116, 0x800);
        for (s, rva, size, offset, flags) in [
            (0x188, 0x1000, 0x200, 0x200, 0x6000_0020),
            (0x1b0, 0x2000, 0x1000, 0x400, 0x4000_0040),
        ] {
            put32(&mut b, s + 8, size);
            put32(&mut b, s + 12, rva);
            put32(&mut b, s + 16, size);
            put32(&mut b, s + 20, offset);
            put32(&mut b, s + 36, flags);
        }
        put32(&mut b, 0x400 + 20, exports.len() as u32);
        put32(&mut b, 0x400 + 24, exports.len() as u32);
        put32(&mut b, 0x400 + 28, 0x2100);
        put32(&mut b, 0x400 + 32, 0x2200);
        put32(&mut b, 0x400 + 36, 0x2300);
        let mut name = 0x800;
        for (i, (text, function)) in exports.iter().enumerate() {
            let rva = if *function { 0x1000 + i * 8 } else { 0x2900 + i * 8 };
            put32(&mut b, 0x500 + i * 4, rva as u32);
            put32(&mut b, 0x600 + i * 4, (name - 0x400 + 0x2000) as u32);
            put16(&mut b, 0x700 + i * 2, i as u16);
            b[name..name + text.len()].copy_from_slice(text.as_bytes());
            name += text.len() + 1;
        }
        b
    }
    struct Fixture {
        _dir: TempDir,
        root: PathBuf,
        context: Value,
        completion: Value,
        records: BTreeMap<String, Vec<Value>>,
    }
    impl Fixture {
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path().canonicalize().unwrap();
            let mut packages = Vec::new();
            for package in [
                "civ6-autotest-rs", "civ6-native-bridge-rs", "civ6-bootstrap-rs",
            ] {
                let manifest = root.join("tools").join(package).join("Cargo.toml");
                fs::create_dir_all(manifest.parent().unwrap()).unwrap();
                fs::write(&manifest, b"source fixture").unwrap();
                packages.push(json!({
                    "id":format!("fixture:{package}"),"name":package,"version":"0.1.0",
                    "manifest_path":manifest,
                }));
            }
            let mut records: BTreeMap<String, Vec<Value>> =
                [("bins".into(), Vec::new()), ("dlls".into(), Vec::new())].into();
            for role in ROLES {
                let path = root.join(role.relative);
                fs::create_dir_all(path.parent().unwrap()).unwrap();
                fs::write(&path, image(role.dll, role.exports)).unwrap();
                let kind = if role.dll { vec!["cdylib", "rlib"] } else { vec!["bin"] };
                records.get_mut(role.phase).unwrap().push(json!({
                    "reason":"compiler-artifact","package_id":format!("fixture:{}",role.package),
                    "manifest_path":root.join(role.manifest),
                    "target":{"name":role.target,"kind":kind,"crate_types":kind},
                    "profile":{"opt_level":"3","debuginfo":0,"debug_assertions":false,
                        "overflow_checks":false,"test":false},
                    "features":[],"filenames":[path],
                    "executable":if role.dll { Value::Null } else { json!(path) },"fresh":false,
                }));
            }
            let source_files = json!([
                {"name":"tools/civ6-autotest-rs/Cargo.toml","bytes":14,"sha256":digest(b"source fixture"),
                    "git_blob_sha1":"8ae520e15adb9d4ec5d505666601dd9c0e13185d"},
                {"name":"tools/civ6-native-bridge-rs/Cargo.toml","bytes":14,"sha256":digest(b"source fixture"),
                    "git_blob_sha1":"8ae520e15adb9d4ec5d505666601dd9c0e13185d"},
                {"name":"tools/civ6-bootstrap-rs/Cargo.toml","bytes":14,"sha256":digest(b"source fixture"),
                    "git_blob_sha1":"8ae520e15adb9d4ec5d505666601dd9c0e13185d"},
            ]);
            let dependency_files = json!([{"name":"Cargo.toml","bytes":3,"sha256":digest(b"dep")}]);
            let source_digest = public_file_inventory(&source_files).unwrap().1;
            let dependency_digest = public_file_inventory(&dependency_files).unwrap().1;
            let profile_environment: Vec<_> = PROFILE_DEFAULT_KEYS.iter()
                .map(|name| json!({"name":name,"present":false,"sha256":null})).collect();
            let inputs = json!({
                "cargo_lock_sha256":"a".repeat(64),"source_inventory_sha256":source_digest,
                "source_file_count":3,"configs":[],"profile_environment":profile_environment,
                "vendors":[{"name":"serde","version":"1.0.0","source_kind":"crates.io",
                    "sha256":dependency_digest,"manifest_sha256":digest(b"dep"),
                    "storage_kind":"registry-extracted","checksum_metadata_sha256":null,
                    "files":dependency_files}],
                "environment":[
                    {"name":"CARGO_INCREMENTAL","sha256":"e".repeat(64)},
                    {"name":"CARGO_PROFILE_DEV_DEBUG","sha256":"e".repeat(64)},
                    {"name":"CARGO_PROFILE_TEST_DEBUG","sha256":"e".repeat(64)},
                ],
                "source_files":source_files,
                "raw_internal_paths":["C:/private/source/not-public"],
            });
            let compiler = json!({
                "toolchain":"1.95.0","host":TARGET,
                "rustc_verbose":format!("rustc 1.95.0 (fixture)\nhost: {TARGET}\nrelease: 1.95.0"),
                "cargo_version":"cargo 1.95.0 (fixture)",
                "rustc_sha256":"a".repeat(64),"cargo_sha256":"b".repeat(64),
                "rustup_manifest_sha256":"c".repeat(64),
                "cargo_dispatcher_sha256":"d".repeat(64),
                "rustup_dispatcher_sha256":"e".repeat(64),
                "rustc_path":"C:/private/compiler/not-public",
            });
            let context = json!({
                "schema":"civ6-product-build-context/v1",
                "repository":"AutoDevJapan/civ6-banseiki","repository_id":1392152644u64,
                "source_sha":"a".repeat(40),"source_tree":"b".repeat(40),
                "source_root":root,"target":TARGET,"tracked_clean":true,
                "compiler":compiler,"inputs":inputs,"workspace_packages":packages,
                "commands":{"bins":phase_argv("bins"),"dlls":phase_argv("dlls")},
                "producer":{"run_id":1,"run_attempt":1,"job":"test","runner_os":"Windows",
                    "runner_arch":"X64","workflow_blob":"c".repeat(40)},
            });
            let compiler_environment = json!({
                "RUSTC":context["compiler"]["rustc_path"],
                "CARGO_BUILD_RUSTC":context["compiler"]["rustc_path"],
                "RUSTC_WRAPPER":"","RUSTC_WORKSPACE_WRAPPER":"",
                "CARGO_ENCODED_RUSTFLAGS":"","RUSTFLAGS":null,"CARGO_BUILD_RUSTFLAGS":null,
                "CARGO_BUILD_RUSTC_WRAPPER":null,
                "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER":null,
            });
            let completion = json!({
                "schema":"civ6-product-build-completion/v1","source_sha":context["source_sha"],
                "source_tree":context["source_tree"],"tracked_clean":true,
                "inputs":context["inputs"],"compiler":context["compiler"],
                "commands":{
                    "bins":{"argv":phase_argv("bins"),"exit_code":0,"collection_complete":true,
                        "native_pid":1,"native_started_utc":"2026-11-16T00:00:00Z",
                        "compiler_environment":compiler_environment,
                        "profile_environment":context["inputs"]["profile_environment"],
                        "stdout_eof":true,"stderr_eof":true,"root_exit_observed":true,"stop_reason":"completed"},
                    "dlls":{"argv":phase_argv("dlls"),"exit_code":0,"collection_complete":true,
                        "native_pid":2,"native_started_utc":"2026-11-16T00:00:01Z",
                        "compiler_environment":compiler_environment,
                        "profile_environment":context["inputs"]["profile_environment"],
                        "stdout_eof":true,"stderr_eof":true,"root_exit_observed":true,"stop_reason":"completed"},
                },
            });
            Self { _dir: dir, root, context, completion, records }
        }
        fn package(&mut self) -> Result<PathBuf, String> {
            let context_bytes = serde_json::to_vec(&self.context).unwrap();
            fs::write(self.root.join("context.json"), &context_bytes).unwrap();
            self.completion["context_sha256"] = json!(digest(&context_bytes));
            for phase in ["bins", "dlls"] {
                let mut bytes = Vec::new();
                for record in &self.records[phase] {
                    bytes.extend(serde_json::to_vec(record).unwrap());
                    bytes.push(b'\n');
                }
                bytes.extend(b"ordinary build tool output\n");
                bytes.extend(b"{\"reason\":\"build-finished\",\"success\":true}\n");
                fs::write(self.root.join(format!("{phase}.jsonl")), &bytes).unwrap();
                self.completion["commands"][phase]["cargo_json_sha256"] = json!(digest(&bytes));
                fs::write(self.root.join(format!("{phase}.jsonl.stderr.raw")), b"").unwrap();
                self.completion["commands"][phase]["stderr_sha256"] = json!(digest(b""));
            }
            fs::write(
                self.root.join("completion.json"),
                serde_json::to_vec(&self.completion).unwrap(),
            ).unwrap();
            let output = self.root.join("bundle");
            write_bundle(
                &self.root.join("context.json"), &self.root.join("bins.jsonl"),
                &self.root.join("dlls.jsonl"), &self.root.join("completion.json"), &output,
            )?;
            Ok(output)
        }
    }

    #[test]
    fn product_bundle_uses_five_actual_roles_and_excludes_raw_private_context() {
        let mut f = Fixture::new();
        f.records.get_mut("bins").unwrap().push(json!({
            "reason":"compiler-artifact","package_id":"dependency:macro","target":{"name":"macro",
            "kind":["proc-macro"],"crate_types":["proc-macro"]},
            "filenames":["C:/private/dependency/not-opened.dll"],"fresh":true,
        }));
        let output = f.package().unwrap();
        let names: BTreeSet<_> = fs::read_dir(&output).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(names.len(), 7);
        for role in ROLES {
            assert_eq!(fs::read(output.join(role.name)).unwrap(), fs::read(f.root.join(role.relative)).unwrap());
        }
        let bytes = fs::read(output.join("manifest.json")).unwrap();
        let m: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(m["files"].as_array().unwrap().len(), 5);
        assert_eq!(m["source_sha"], f.context["source_sha"]);
        assert_eq!(m["source_tree"], f.context["source_tree"]);
        let cargo = fs::read_to_string(output.join("cargo-artifacts.json")).unwrap();
        assert_eq!(m["cargo_artifacts_sha256"], digest(cargo.as_bytes()));
        let selected: Value = serde_json::from_str(&cargo).unwrap();
        assert_eq!(selected["inputs_before"], selected["inputs_after"]);
        assert_eq!(selected["inputs_before"]["source_files"].as_array().unwrap().len(), 3);
        assert_eq!(selected["records"].as_array().unwrap().len(), 5);
        assert!(selected["records"][0]["package"]["id_sha256"].as_str().is_some_and(|s| hex(s, 64)));
        assert_eq!(selected["records"][0]["profile"]["opt_level"], "3");
        assert!(!String::from_utf8(bytes).unwrap().contains("C:/private"));
        assert!(!cargo.contains("C:/private"));
        assert!(!cargo.contains(&f.root.to_string_lossy().to_string()));
        assert!(f.package().is_err(), "an existing bundle must never be overwritten");
    }

    #[test]
    fn product_bundle_rejects_changed_inputs_source_command_or_failed_collection() {
        for mode in 0..14 {
            let mut f = Fixture::new();
            match mode {
                0 => f.completion["source_sha"] = json!("f".repeat(40)),
                1 => f.completion["inputs"]["cargo_lock_sha256"] = json!("f".repeat(64)),
                2 => f.completion["commands"]["dlls"]["exit_code"] = json!(1),
                3 => f.completion["commands"]["bins"]["collection_complete"] = json!(false),
                4 => f.context["commands"]["bins"] = json!(["cargo", "test"]),
                5 => f.context["compiler"]["host"] = json!("wrong-host"),
                6 => f.completion["commands"]["bins"]["native_pid"] = json!(0),
                7 => f.completion["commands"]["bins"]["native_started_utc"] = Value::Null,
                8 => f.completion["commands"]["bins"]["compiler_environment"]["RUSTC"] =
                    json!("a different compiler"),
                9 => f.completion["commands"]["bins"]["compiler_environment"]["RUSTC_WRAPPER"] =
                    json!("a wrapper"),
                10 => f.completion["commands"]["bins"]["compiler_environment"]
                    ["CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER"] = json!("a workspace wrapper"),
                11 => f.completion["commands"]["bins"]["compiler_environment"]
                    ["CARGO_ENCODED_RUSTFLAGS"] = json!("--other-flags"),
                12 => f.completion["commands"]["bins"]["compiler_environment"]
                    ["RUSTFLAGS"] = json!("--other-flags"),
                _ => f.completion["commands"]["bins"]["compiler_environment"]
                    ["CARGO_BUILD_RUSTFLAGS"] = json!("--other-flags"),
            }
            assert!(f.package().is_err(), "mode {mode}");
            assert!(!f.root.join("bundle").exists());
        }
    }

    #[test]
    fn product_bundle_rejects_missing_duplicate_wrong_kind_and_wrong_output() {
        for mode in 0..8 {
            let mut f = Fixture::new();
            match mode {
                0 => { f.records.get_mut("bins").unwrap().remove(0); }
                1 => {
                    let duplicate = f.records["bins"][0].clone();
                    f.records.get_mut("bins").unwrap().push(duplicate);
                }
                2 => f.records.get_mut("dlls").unwrap()[0]["target"]["kind"] = json!(["proc-macro"]),
                3 => {
                    let alias = f.root.join("wrong").join(ROLES[0].name);
                    fs::create_dir_all(alias.parent().unwrap()).unwrap();
                    fs::write(&alias, image(false, &[])).unwrap();
                    f.records.get_mut("bins").unwrap()[0]["filenames"] = json!([alias]);
                }
                4 => f.records.get_mut("bins").unwrap()[0]["package_id"] = json!("unrelated"),
                5 => f.records.get_mut("bins").unwrap()[0]["executable"] = Value::Null,
                6 => f.records.get_mut("bins").unwrap()[0]["profile"] = json!({"test":false}),
                _ => f.records.get_mut("bins").unwrap()[0]["features"] = json!(["private/path"]),
            }
            assert!(f.package().is_err(), "mode {mode}");
            assert!(!f.root.join("bundle").exists());
        }
    }

    #[test]
    fn product_pe_rejects_wrong_machine_kind_missing_forwarded_and_data_exports() {
        for mode in 0..5 {
            let mut b = image(true, BOOTSTRAP);
            match mode {
                0 => put16(&mut b, 0x84, 0x14c),
                1 => put16(&mut b, 0x96, 0x22),
                2 => b[0x800] = b'X',
                3 => put32(&mut b, 0x500, 0x2040),
                _ => put32(&mut b, 0x500, 0x2900),
            }
            assert!(pe_exports(&b, true, BOOTSTRAP).is_err(), "mode {mode}");
        }
        let mut zero_data = image(true, BOOTSTRAP);
        put32(&mut zero_data, 0x1b0 + 16, 0x800);
        pe_exports(&zero_data, true, BOOTSTRAP).unwrap();
        assert!(pe_exports(&zero_data[..0x400], true, BOOTSTRAP).is_err());
    }

    #[test]
    fn product_cargo_stream_requires_complete_successful_json_records() {
        for bytes in [
            b"ordinary output\n".as_slice(),
            b"{\"reason\":\"build-finished\",\"success\":false}\n".as_slice(),
            b"{\"reason\":\"build-finished\",\"success\":true}\n{".as_slice(),
            b"{\"reason\":\"build-finished\",\"success\":true}\n{\"reason\":\"build-finished\",\"success\":true}\n".as_slice(),
        ] {
            assert!(cargo_records(bytes).is_err());
        }
        let records = cargo_records(
            b"ordinary output\n{\"reason\":\"build-finished\",\"success\":true}\n",
        ).unwrap();
        assert!(records.is_empty());
    }


    #[test]
    fn product_pe_requires_executable_image_bit_for_exe_and_dll() {
        for dll in [false, true] {
            let exports: &[(&str, bool)] = if dll { BOOTSTRAP } else { &[] };
            let mut bytes = image(dll, exports);
            pe_exports(&bytes, dll, exports).unwrap();
            let characteristics = u16_at(&bytes, 0x96).unwrap();
            put16(&mut bytes, 0x96, characteristics & !0x0002);
            let error = pe_exports(&bytes, dll, exports).unwrap_err();
            assert!(error.contains("executable image"), "{error}");
        }
    }

    fn compiler_control_command(program: &Path) -> std::process::Command {
        let mut command = std::process::Command::new(program);
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x0800_0000);
        }
        command
    }

    fn compiler_control_tool(name: &str) -> PathBuf {
        let output = compiler_control_command(Path::new("rustup"))
            .args(["which", "--toolchain", "1.95.0", name])
            .output()
            .unwrap();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        PathBuf::from(String::from_utf8(output.stdout).unwrap().trim())
    }

    #[test]
    fn product_fixed_rustc_overrides_cargo_config_and_environment_aliases() {
        let cargo = compiler_control_tool("cargo");
        let rustc = compiler_control_tool("rustc");
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        fs::create_dir(root.join("src")).unwrap();
        fs::create_dir(root.join(".cargo")).unwrap();
        fs::create_dir(root.join("cargo-home")).unwrap();
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname='compiler-selection-control'\nversion='0.1.0'\nedition='2021'\n[workspace]\n",
        ).unwrap();
        fs::write(root.join("src/lib.rs"), "pub const CONTROL: u8 = 7;\n").unwrap();
        let poison_name = "unavailable_compiler_selection_control";
        let poison = root.join(poison_name);
        let quoted = serde_json::to_string(&poison.to_string_lossy()).unwrap();
        let config = root.join(".cargo/config.toml");
        let aliases = [
            "RUSTC", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER",
            "CARGO_BUILD_RUSTC", "CARGO_BUILD_RUSTC_WRAPPER",
            "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
        ];
        let command = || {
            let mut command = compiler_control_command(&cargo);
            command.current_dir(root)
                .args(["check", "--offline", "--message-format=json"])
                .arg("--manifest-path").arg(root.join("Cargo.toml"))
                .env("CARGO_HOME", root.join("cargo-home"))
                .env("CARGO_TARGET_DIR", root.join("target"))
                .env_remove("CARGO_BUILD_TARGET")
                .env_remove("CARGO_BUILD_TARGET_DIR")
                .env_remove("CARGO_BUILD_RUSTFLAGS")
                .env_remove("RUSTFLAGS")
                .env_remove("CARGO_ENCODED_RUSTFLAGS");
            for alias in aliases {
                command.env_remove(alias);
            }
            command
        };
        for mode in 0..6 {
            let mut unbound = command();
            if mode < 3 {
                fs::write(&config, "").unwrap();
                unbound.env(aliases[mode + 3], &poison);
            } else {
                let key = ["rustc", "rustc-wrapper", "rustc-workspace-wrapper"][mode - 3];
                fs::write(&config, format!("[build]\n{key}={quoted}\n")).unwrap();
            }
            let result = unbound.output().unwrap();
            assert!(!result.status.success(), "unbound selection {mode}");
            assert!(
                String::from_utf8_lossy(&result.stderr).contains(poison_name),
                "mode {mode}: {}", String::from_utf8_lossy(&result.stderr),
            );
        }
        fs::write(
            &config,
            format!(
                "[build]\nrustc={quoted}\nrustc-wrapper={quoted}\nrustc-workspace-wrapper={quoted}\n",
            ),
        ).unwrap();
        let mut flags = command();
        fs::write(&config, "").unwrap();
        flags.env("RUSTC", &rustc)
            .env("CARGO_BUILD_RUSTFLAGS", "--unavailable-compiler-flags-control");
        let rejected = flags.output().unwrap();
        assert!(!rejected.status.success());
        assert!(String::from_utf8_lossy(&rejected.stderr)
            .contains("unavailable-compiler-flags-control"));
        fs::write(
            &config,
            format!("[build]\nrustc={quoted}\nrustc-wrapper={quoted}\nrustc-workspace-wrapper={quoted}\nrustflags=['--unavailable-compiler-flags-control']\n"),
        ).unwrap();
        let mut bound = command();
        for alias in &aliases[3..] {
            bound.env(alias, &poison);
        }
        bound.env("CARGO_BUILD_RUSTFLAGS", "--unavailable-compiler-flags-control");
        // The same explicit child environment is used by the CI producer.
        bound.env("RUSTC", &rustc).env("CARGO_BUILD_RUSTC", &rustc)
            .env("RUSTC_WRAPPER", "").env("RUSTC_WORKSPACE_WRAPPER", "")
            .env_remove("CARGO_BUILD_RUSTC_WRAPPER")
            .env_remove("CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER")
            .env("CARGO_ENCODED_RUSTFLAGS", "")
            .env_remove("RUSTFLAGS")
            .env_remove("CARGO_BUILD_RUSTFLAGS");
        let result = bound.output().unwrap();
        assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
        let artifacts = cargo_records(&result.stdout).unwrap();
        assert!(artifacts.iter().any(|(record, _)| {
            record["target"]["name"] == "compiler_selection_control"
                && record["fresh"] == false
        }), "the fixed compiler must actually produce the control crate");
    }

    #[test]
    fn product_input_inventory_preserves_complete_relative_facts_and_rejects_escapes() {
        let rows = json!([{"name":"src/lib.rs","bytes":3,"sha256":"0".repeat(64)}]);
        let (safe, hash) = public_file_inventory(&rows).unwrap();
        assert_eq!(safe[0]["name"], rows[0]["name"]);
        assert!(safe[0]["git_blob_sha1"].is_null());
        assert_eq!(hash, "1330e3a6132b2d61ac33e1be474cb10428f12cedaad825f94a209631fcc1e4e1");
        for name in ["C:/private/input", "../other", "/root", "a\\b", "a/./b", "a//b", "a\nb"] {
            let bad = json!([{"name":name,"bytes":3,"sha256":"0".repeat(64)}]);
            assert!(public_file_inventory(&bad).is_err(), "{name:?}");
        }
        assert!(public_file_inventory(&json!([rows[0], rows[0]])).is_err());
        let mut bad_git = rows.clone();
        bad_git[0]["git_blob_sha1"] = json!(false);
        assert!(public_file_inventory(&bad_git).is_err());
    }

    #[test]
    fn product_registry_inputs_do_not_require_directory_checksum_metadata() {
        let f = Fixture::new();
        public_inputs(&f.context["inputs"]).unwrap();
        let mut inputs = f.context["inputs"].clone();
        inputs["vendors"][0]["storage_kind"] = json!("directory");
        assert!(public_inputs(&inputs).is_err());
        inputs["vendors"][0]["checksum_metadata_sha256"] = json!("a".repeat(64));
        inputs["vendors"][0]["files"].as_array_mut().unwrap().push(
            json!({"name":".cargo-checksum.json","bytes":1,"sha256":"a".repeat(64)}),
        );
        inputs["vendors"][0]["sha256"] =
            json!(public_file_inventory(&inputs["vendors"][0]["files"]).unwrap().1);
        public_inputs(&inputs).unwrap();
        inputs["source_files"][0]["name"] = json!("../private");
        assert!(public_inputs(&inputs).is_err());
    }

    #[test]
    fn product_profile_inputs_distinguish_absence_empty_change_and_child_mismatch() {
        let f = Fixture::new();
        let baseline = &f.context["inputs"]["profile_environment"];
        let before = public_profile_environment(baseline).unwrap();
        let mut changed = baseline.clone();
        for row in changed.as_array_mut().unwrap() {
            if row["name"] == "CARGO_PROFILE_RELEASE_PANIC" {
                row["present"] = json!(true);
                row["sha256"] = json!(digest(b"abort"));
            }
        }
        assert_ne!(public_profile_environment(&changed).unwrap(), before);
        let mut empty = changed.clone();
        for row in empty.as_array_mut().unwrap() {
            if row["name"] == "CARGO_PROFILE_RELEASE_PANIC" {
                row["sha256"] = json!(digest(b""));
            }
        }
        assert_ne!(public_profile_environment(&empty).unwrap(), before);
        assert_ne!(public_profile_environment(&empty).unwrap(), public_profile_environment(&changed).unwrap());
        let mut mismatched = Fixture::new();
        mismatched.completion["commands"]["bins"]["profile_environment"] = changed;
        assert!(mismatched.package().is_err());
        assert!(!mismatched.root.join("bundle").exists());
    }

    #[cfg(windows)]
    fn run_capture_fixture(case: &str, profile: &str) -> (TempDir, Vec<Value>, String) {
        let directory = tempfile::tempdir().unwrap();
        let executable = std::env::current_exe().unwrap();
        let mut command = compiler_control_command(Path::new("pwsh"));
        command.args(["-NoProfile", "-NonInteractive", "-File"])
            .arg(crate::repo_root().join(".github/scripts/civ6-product-artifacts.ps1"))
            .args(["-Phase", "FixtureCapture", "-FixtureRoot"])
            .arg(directory.path())
            .arg("-FixtureProgram").arg(&executable)
            .args(["-FixtureCase", case, "-FixtureProfile", profile]);
        let output = command.output().unwrap();
        assert!(output.status.success(), "capture fixture driver failed");
        let stdout = String::from_utf8(output.stdout).unwrap();
        let stderr = String::from_utf8(output.stderr).unwrap();
        assert!(!stdout.contains("C:/private/stdout-sentinel"), "stdout marker escaped");
        assert!(!stdout.contains("C:/private/stderr-sentinel"), "stderr marker escaped");
        assert!(!stderr.contains("C:/private/"), "private data escaped driver stderr");
        let line = stdout.lines().find_map(|line| line.strip_prefix("CIV_PRODUCT_FIXTURE_RESULT ")).unwrap();
        let value: Value = serde_json::from_str(line).unwrap();
        let records = if let Some(records) = value.as_array() {
            records.clone()
        } else {
            vec![value]
        };
        (directory, records, stdout)
    }

    #[cfg(windows)]
    #[test]
    fn product_capture_drains_both_large_streams_and_preserves_failure_exit() {
        for (case, expected) in [("success", 0), ("failure", 23)] {
            let (directory, events, _) = run_capture_fixture(case, "absent");
            assert_eq!(events.len(), 1);
            assert_eq!(events[0]["exit_code"], expected);
            assert_eq!(events[0]["complete"], true);
            assert_eq!(events[0]["reason"], "completed");
            let stdout = fs::read(directory.path().join("fixture.stdout.raw")).unwrap();
            let stderr = fs::read(directory.path().join("fixture.stdout.raw.stderr.raw")).unwrap();
            assert!(stdout.len() >= 2 * 1024 * 1024);
            assert!(stderr.len() >= 2 * 1024 * 1024);
            assert!(stdout.windows(b"C:/private/stdout-sentinel".len()).any(|b| b == b"C:/private/stdout-sentinel"));
            assert!(stderr.windows(b"C:/private/stderr-sentinel".len()).any(|b| b == b"C:/private/stderr-sentinel"));
            for (bytes, marker, fill) in [
                (&stdout, b"C:/private/stdout-sentinel\n".as_slice(), b'O'),
                (&stderr, b"C:/private/stderr-sentinel\n".as_slice(), b'E'),
            ] {
                let start = bytes.windows(marker.len()).position(|b| b == marker).unwrap() + marker.len();
                assert_eq!(&bytes[start..start + 2 * 1024 * 1024], vec![fill; 2 * 1024 * 1024]);
            }
            let result: Value = serde_json::from_slice(
                &fs::read(directory.path().join("fixture.stdout.raw.result.json")).unwrap(),
            ).unwrap();
            assert_eq!(result["stdout_sha256"], digest(&stdout));
            assert_eq!(result["stderr_sha256"], digest(&stderr));
        }
    }

    #[cfg(windows)]
    #[test]
    fn product_capture_cancel_and_deadline_keep_partial_raw_and_allow_next_capture() {
        for case in ["cancel", "deadline"] {
            let (directory, events, _) = run_capture_fixture(case, "absent");
            assert_eq!(events.len(), 2);
            assert_eq!(events[0]["complete"], false);
            assert_eq!(events[0]["reason"], if case == "cancel" { "cancelled" } else { "deadline" });
            assert_eq!(events[0]["root_exit"], true);
            assert!(events[0]["exit_code"].as_i64().is_some());
            assert_eq!(events[1]["complete"], true);
            assert_eq!(events[1]["exit_code"], 0);
            assert!(directory.path().join("fixture.stdout.raw").exists());
            assert!(directory.path().join("fixture.stdout.raw.stderr.raw").exists());
            assert!(directory.path().join("next.stdout.raw").exists());
        }
    }

    #[cfg(windows)]
    #[test]
    fn product_capture_records_actual_child_release_profile_overrides() {
        let (_baseline_dir, baseline, _) = run_capture_fixture("success", "absent");
        let (_changed_dir, changed, _) = run_capture_fixture("success", "changed");
        for (name, value) in [
            ("CARGO_PROFILE_RELEASE_PANIC", "abort"),
            ("CARGO_PROFILE_RELEASE_LTO", "true"),
            ("CARGO_PROFILE_RELEASE_CODEGEN_UNITS", "1"),
            ("CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_CODEGEN_UNITS", "1"),
        ] {
            let old = baseline[0]["profile_environment"].as_array().unwrap().iter()
                .find(|row| row["name"] == name).unwrap();
            let new = changed[0]["profile_environment"].as_array().unwrap().iter()
                .find(|row| row["name"] == name).unwrap();
            assert_eq!(old["present"], false);
            assert_eq!(old["sha256"], Value::Null);
            assert_eq!(new["present"], true);
            assert_eq!(new["sha256"], digest(value.as_bytes()));
        }
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "owned subprocess helper for dual-stream tests"]
    fn product_capture_fixture_child() {
        use std::io::Write;
        let Ok(case) = std::env::var("CIV6_CAPTURE_FIXTURE_CASE") else { return; };
        let out = std::thread::spawn(|| {
            let mut stream = std::io::stdout().lock();
            stream.write_all(b"C:/private/stdout-sentinel\n").unwrap();
            stream.write_all(&vec![b'O'; 2 * 1024 * 1024]).unwrap();
            stream.flush().unwrap();
        });
        let err = std::thread::spawn(|| {
            let mut stream = std::io::stderr().lock();
            stream.write_all(b"C:/private/stderr-sentinel\n").unwrap();
            stream.write_all(&vec![b'E'; 2 * 1024 * 1024]).unwrap();
            stream.flush().unwrap();
        });
        out.join().unwrap();
        err.join().unwrap();
        if case == "wait" {
            loop { std::thread::sleep(std::time::Duration::from_millis(100)); }
        }
        if case == "failure" { std::process::exit(23); }
    }

    #[cfg(windows)]
    #[test]
    fn product_directory_capture_accepts_matching_empty_map_and_rejects_listed_mismatch() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        let package = root.join("vendor/fixture");
        fs::create_dir_all(package.join("src")).unwrap();
        fs::write(package.join("Cargo.toml"), "[package]\nname='fixture'\nversion='0.1.0'\nedition='2021'\n").unwrap();
        fs::write(package.join("src/lib.rs"), "pub const VALUE: u8 = 1;\n").unwrap();
        let checksum_path = package.join(".cargo-checksum.json");
        fs::write(&checksum_path, serde_json::to_vec(
            &json!({"package":"0".repeat(64),"files":{}}),
        ).unwrap()).unwrap();
        let project = root.join("project");
        fs::create_dir_all(project.join(".cargo")).unwrap();
        fs::create_dir_all(project.join("src")).unwrap();
        fs::create_dir(root.join("cargo-home")).unwrap();
        fs::write(project.join("Cargo.toml"),
            "[package]\nname='inventory-control'\nversion='0.1.0'\nedition='2021'\n[dependencies]\nfixture='=0.1.0'\n[workspace]\n").unwrap();
        fs::write(project.join("src/lib.rs"), "pub fn value() -> u8 { fixture::VALUE }\n").unwrap();
        fs::write(project.join("Cargo.lock"), format!(
            "version = 4\n[[package]]\nname='inventory-control'\nversion='0.1.0'\ndependencies=['fixture']\n[[package]]\nname='fixture'\nversion='0.1.0'\nsource='registry+https://github.com/rust-lang/crates.io-index'\nchecksum='{}'\n", "0".repeat(64),
        )).unwrap();
        let quoted_vendor = serde_json::to_string(&root.join("vendor").to_string_lossy()).unwrap();
        fs::write(project.join(".cargo/config.toml"), format!(
            "[source.crates-io]\nreplace-with='local'\n[source.local]\ndirectory={quoted_vendor}\n",
        )).unwrap();
        let cargo = compiler_control_tool("cargo");
        let rustc = compiler_control_tool("rustc");
        let mut metadata = compiler_control_command(&cargo);
        metadata.current_dir(&project)
            .args(["metadata", "--offline", "--locked", "--format-version", "1"])
            .env("CARGO_HOME", root.join("cargo-home"))
            .env("RUSTC", &rustc).env("CARGO_BUILD_RUSTC", &rustc)
            .env("RUSTC_WRAPPER", "").env("RUSTC_WORKSPACE_WRAPPER", "")
            .env_remove("CARGO_BUILD_RUSTC_WRAPPER").env_remove("CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER")
            .env("CARGO_ENCODED_RUSTFLAGS", "");
        let accepted = metadata.output().unwrap();
        assert!(accepted.status.success(), "normal Cargo empty-map fixture rejected");
        let capture = || {
            let mut command = compiler_control_command(Path::new("pwsh"));
            command.args(["-NoProfile", "-NonInteractive", "-File"])
                .arg(crate::repo_root().join(".github/scripts/civ6-product-artifacts.ps1"))
                .args(["-Phase", "FixtureInventory", "-FixtureStorage", "directory", "-FixtureRoot"])
                .arg(root);
            command.output().unwrap()
        };
        let empty = capture();
        assert!(empty.status.success(), "empty-map input capture rejected");
        let empty_stdout = String::from_utf8(empty.stdout).unwrap();
        let line = empty_stdout.lines().find_map(|l| l.strip_prefix("CIV_PRODUCT_INVENTORY_FIXTURE ")).unwrap();
        let empty_fact: Value = serde_json::from_str(line).unwrap();
        assert!(empty_fact["files"].as_array().unwrap().len() >= 3);
        let listed = json!({
            "Cargo.toml":digest(&fs::read(package.join("Cargo.toml")).unwrap()),
            "src/lib.rs":digest(&fs::read(package.join("src/lib.rs")).unwrap()),
        });
        fs::write(&checksum_path, serde_json::to_vec(
            &json!({"package":"0".repeat(64),"files":listed}),
        ).unwrap()).unwrap();
        assert!(capture().status.success(), "ordinary vendor checksum rejected");
        fs::write(package.join("src/lib.rs"), "pub const VALUE: u8 = 2;\n").unwrap();
        assert!(!capture().status.success(), "listed source mismatch was accepted");
        fs::write(&checksum_path, serde_json::to_vec(
            &json!({"package":"0".repeat(64),"files":{}}),
        ).unwrap()).unwrap();
        let changed = capture();
        assert!(changed.status.success(), "independent inventory must support empty maps");
        let text = String::from_utf8(changed.stdout).unwrap();
        let line = text.lines().find_map(|l| l.strip_prefix("CIV_PRODUCT_INVENTORY_FIXTURE ")).unwrap();
        let changed_fact: Value = serde_json::from_str(line).unwrap();
        assert_ne!(changed_fact["sha256"], empty_fact["sha256"]);
    }

    #[test]
    fn product_context_public_projection_drops_unknown_raw_paths() {
        let f = Fixture::new();
        let inputs = public_inputs(&f.context["inputs"]).unwrap().to_string();
        let compiler = public_compiler(&f.context).unwrap().to_string();
        assert!(inputs.contains("source_files"));
        assert!(!inputs.contains("raw_internal_paths"));
        assert!(!inputs.contains("C:/private"));
        assert!(!compiler.contains("rustc_path"));
        assert!(!compiler.contains("C:/private"));
        let mut bad = f.context["inputs"].clone();
        bad["configs"] = json!([{"scope":"ancestor-C:/private","name":"config",
            "bytes":1,"sha256":"a".repeat(64)}]);
        assert!(public_inputs(&bad).is_err());
    }
}
