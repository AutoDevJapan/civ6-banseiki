//! Prepare owned local Banseiki build artifacts from installed optional DLC.
//! Official data is generated only in deployment copies, never tracked or redistributed.
use crate::{read_utf8, sql_quote, Repository};
use anyhow::{bail, ensure, Context, Result};
use roxmltree::{Document, Node};
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

pub const MOD_DIR: &str = "poc/JAC_PoC_BanseikiRuleset";
pub const MODINFO: &str = "JAC_PoC_BanseikiRuleset.modinfo";
pub const DLC_ID: &str = "1B394FE9-23DC-4868-8F0A-5220CB8FB427";
pub const SQL_FILE: &str = "Data/JAC_Banseiki_Ethiopia.sql";
pub const SOURCE_MOD_ID: &str = "bd7a82c5-6b38-4b69-a7f5-8dc9af534249";
pub const OWNER_FILE: &str = "JAC_LocalDlc_Owner.json";
const GENERATOR: &str = "banseiki-local-dlc-prepare";
pub const MAYA_ID: &str = "9DE86512-DE1A-400D-8C0A-AB46EBBF76B9";

#[derive(Clone, Copy)]
pub struct DlcDefinition {
    pub name: &'static str,
    pub id: &'static str,
    pub components: [(&'static str, usize); 2],
}
pub const ETHIOPIA: DlcDefinition = DlcDefinition {
    name: "Ethiopia",
    id: DLC_ID,
    components: [("EthiopiaGameplay", 8), ("EthiopiaGameplay_XP2", 2)],
};
pub const BABYLON: DlcDefinition = DlcDefinition {
    name: "Babylon",
    id: "8424840C-92EF-4426-A9B4-B4E0CB818049",
    components: [("BabylonGameplay", 8), ("BabylonGameplay_XP2", 1)],
};
pub const DLCS: [DlcDefinition; 2] = [ETHIOPIA, BABYLON];

struct DatabasePatch {
    id: String,
    dlc_id: &'static str,
    mod_in_use: Option<&'static str>,
    sql: String,
}
impl DatabasePatch {
    fn file(&self) -> String {
        format!("Data/{}.sql", self.id)
    }
}

fn ident(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

fn literal(value: &str) -> String {
    match value.to_ascii_lowercase().as_str() {
        "true" => "1".into(),
        "false" => "0".into(),
        _ => sql_quote(value),
    }
}

fn fields(node: Node<'_, '_>) -> Result<BTreeMap<String, String>> {
    let mut out: BTreeMap<_, _> = node
        .attributes()
        .map(|a| (a.name().to_owned(), literal(a.value())))
        .collect();
    for child in node.children().filter(Node::is_element) {
        ensure!(
            !child.children().any(|n| n.is_element()),
            "nested XML field"
        );
        let name = child.tag_name().name().to_owned();
        ensure!(!out.contains_key(&name), "duplicate XML field {name}");
        out.insert(name, literal(child.text().unwrap_or("")));
    }
    ensure!(!out.is_empty(), "empty XML operation");
    Ok(out)
}

fn predicate(fields: &BTreeMap<String, String>) -> String {
    fields
        .iter()
        .map(|(k, v)| format!("{} IS {v}", ident(k)))
        .collect::<Vec<_>>()
        .join(" AND ")
}

/// Preserve official operation order. Row uses both conflict handling (keyed tables)
/// and a full-row existence test (tables without a UNIQUE key). No REPLACE cascades.
pub fn xml_sql(source: &str) -> Result<String> {
    let doc = Document::parse(source.trim_start_matches('\u{feff}'))?;
    ensure!(
        doc.root_element().has_tag_name("GameInfo"),
        "expected GameInfo XML"
    );
    let mut out = String::new();
    for table in doc.root_element().children().filter(Node::is_element) {
        let table_name = ident(table.tag_name().name());
        for op in table.children().filter(Node::is_element) {
            let sql = match op.tag_name().name() {
                "Row" | "InsertOrIgnore" => {
                    let row = fields(op)?;
                    let columns = row.keys().map(|s| ident(s)).collect::<Vec<_>>().join(",");
                    let values = row.values().cloned().collect::<Vec<_>>().join(",");
                    format!("INSERT OR IGNORE INTO {table_name} ({columns}) SELECT {values} WHERE NOT EXISTS (SELECT 1 FROM {table_name} WHERE {});", predicate(&row))
                }
                "Delete" => format!(
                    "DELETE FROM {table_name} WHERE {};",
                    predicate(&fields(op)?)
                ),
                "Update" => {
                    let children: Vec<_> = op.children().filter(Node::is_element).collect();
                    ensure!(children.len() == 2, "Update requires exactly Where and Set");
                    let wh = children
                        .iter()
                        .find(|n| n.has_tag_name("Where"))
                        .context("Update Where")?;
                    let set = children
                        .iter()
                        .find(|n| n.has_tag_name("Set"))
                        .context("Update Set")?;
                    let changes = fields(*set)?
                        .iter()
                        .map(|(k, v)| format!("{}={v}", ident(k)))
                        .collect::<Vec<_>>()
                        .join(",");
                    format!(
                        "UPDATE {table_name} SET {changes} WHERE {};",
                        predicate(&fields(*wh)?)
                    )
                }
                name => bail!("unsupported official XML operation {name}"),
            };
            out.push_str(&sql);
            out.push('\n');
        }
    }
    Ok(out)
}

/// Read only the two non-mode gameplay components, in official priority order.
pub fn sources(manifest: &str) -> Result<Vec<String>> {
    dlc_sources(manifest, ETHIOPIA)
}

pub fn dlc_sources(manifest: &str, dlc: DlcDefinition) -> Result<Vec<String>> {
    let mut files = Vec::new();
    for (id, count) in dlc.components {
        files.extend(component_sources(manifest, dlc, id, count)?);
    }
    Ok(files)
}

pub fn component_sources(
    manifest: &str,
    dlc: DlcDefinition,
    id: &str,
    count: usize,
) -> Result<Vec<String>> {
    let doc = Document::parse(manifest.trim_start_matches('\u{feff}'))?;
    ensure!(
        doc.root_element()
            .attribute("id")
            .is_some_and(|id| id.eq_ignore_ascii_case(dlc.id)),
        "not the {} DLC",
        dlc.name
    );
    let mut paths = Vec::new();
    let actions: Vec<_> = doc
        .descendants()
        .filter(|n| n.has_tag_name("UpdateDatabase") && n.attribute("id") == Some(id))
        .collect();
    ensure!(actions.len() == 1, "missing or duplicate {id}");
    let action = actions[0];
    let order = action
        .children()
        .find(|n| n.has_tag_name("Properties"))
        .and_then(|n| n.children().find(|n| n.has_tag_name("LoadOrder")))
        .and_then(|n| n.text())
        .unwrap_or("0")
        .parse::<i64>()?;
    ensure!(order == 0, "{id}: unsupported official LoadOrder {order}");
    let mut files: Vec<_> = action
        .children()
        .filter(|n| n.has_tag_name("File"))
        .map(|n| {
            Ok((
                n.attribute("Priority").unwrap_or("0").parse::<i64>()?,
                n.text().context("file path")?.to_owned(),
            ))
        })
        .collect::<Result<_>>()?;
    files.sort_by_key(|(priority, _)| std::cmp::Reverse(*priority));
    for (_, path) in files {
        ensure!(
            path.starts_with("Data/")
                && path.ends_with(".xml")
                && !path.contains("..")
                && !path.contains('\\'),
            "unexpected DLC path {path}"
        );
        paths.push(path);
    }
    ensure!(
        paths.len() == count,
        "{id}: expected {count} files, got {}",
        paths.len()
    );
    Ok(paths)
}

pub fn gameplay_sql(game: &Path) -> Result<String> {
    dlc_gameplay_sql(game, ETHIOPIA)
}

pub fn dlc_gameplay_sql(game: &Path, definition: DlcDefinition) -> Result<String> {
    let dir = game.join("DLC").join(definition.name);
    compile_files(
        &dir,
        &dlc_sources(
            &read_utf8(&dir.join(format!("{}.modinfo", definition.name)))?,
            definition,
        )?,
    )
}

fn compile_files(dir: &Path, files: &[String]) -> Result<String> {
    let mut out = "-- Locally generated from installed DLC; do not redistribute.\n\
        -- Repair base rows then replay XP2 updates after official components.\n"
        .to_owned();
    for file in files {
        out.push_str(&format!("-- {file}\n"));
        out.push_str(
            &xml_sql(&read_utf8(&dir.join(file))?).with_context(|| format!("convert {file}"))?,
        );
    }
    Ok(out)
}

/// Modify only the prepared copy; keep the original source MOD usable without DLC.
pub fn modinfo(source: &str) -> Result<String> {
    manifest_with_patches(
        source,
        &[DatabasePatch {
            id: "JAC_Banseiki_Ethiopia".into(),
            dlc_id: ETHIOPIA.id,
            mod_in_use: None,
            sql: String::new(),
        }],
    )
}

fn manifest_with_patches(source: &str, patches: &[DatabasePatch]) -> Result<String> {
    let doc = Document::parse(source)?;
    ensure!(
        !doc.descendants()
            .any(|n| n.attribute("id").is_some_and(
                |id| id == "JAC_Banseiki_Ethiopia" || id.starts_with("JAC_Banseiki_Babylon")
            )),
        "MOD already prepared"
    );
    ensure!(
        doc.root_element()
            .attribute("id")
            .is_some_and(|id| id.eq_ignore_ascii_case(SOURCE_MOD_ID)),
        "unexpected source MOD id"
    );
    let mut criteria = String::new();
    let mut actions = String::new();
    let mut files = String::new();
    for patch in patches {
        let id = &patch.id;
        let dlc_id = patch.dlc_id;
        let extra = patch
            .mod_in_use
            .map(|id| format!("\t\t\t<ModInUse>{id}</ModInUse>\n"))
            .unwrap_or_default();
        criteria.push_str(&format!("\t\t<Criteria id=\"{id}\">\n\t\t\t<RuleSetInUse>RULESET_JAC_BANSEIKI</RuleSetInUse>\n\t\t\t<GameCoreInUse>Expansion2</GameCoreInUse>\n\t\t\t<ModIsEnabled>{dlc_id}</ModIsEnabled>\n{extra}\t\t</Criteria>\n"));
        let file = patch.file();
        let order = if patch.mod_in_use.is_some() { 2 } else { 1 };
        actions.push_str(&format!("\t\t<UpdateDatabase id=\"{id}\" criteria=\"{id}\">\n\t\t\t<Properties><LoadOrder>{order}</LoadOrder></Properties>\n\t\t\t<File>{file}</File>\n\t\t</UpdateDatabase>\n"));
        files.push_str(&format!("\t\t<File>{file}</File>\n"));
    }
    let mut output = source.to_owned();
    // Babylon is optional for the prepared base MOD. Its existing Heroes actions
    // remain gated, rather than making absent/disabled Babylon a hard dependency.
    for node in doc.descendants().filter(|n| {
        n.has_tag_name("Mod") && n.parent().is_some_and(|p| p.has_tag_name("Dependencies"))
    }) {
        if node
            .attribute("id")
            .is_some_and(|id| id.eq_ignore_ascii_case(BABYLON.id))
        {
            output = output.replace(&source[node.range()], "");
        }
    }
    if let Some(hero) = doc
        .descendants()
        .find(|n| n.has_tag_name("Criteria") && n.attribute("id") == Some("JAC_Banseiki_Heroes"))
    {
        let original = &source[hero.range()];
        let updated = original.replace(
            "</Criteria>",
            &format!(
                "<ModIsEnabled>{}</ModIsEnabled>\n\t\t</Criteria>",
                BABYLON.id
            ),
        );
        output = output.replace(original, &updated);
    }
    for (closing, addition) in [
        ("\t</ActionCriteria>", criteria),
        ("\t</InGameActions>", actions),
        ("\t</Files>", files),
    ] {
        ensure!(
            output.matches(closing).count() == 1,
            "missing or duplicate {closing}"
        );
        output = output.replace(closing, &(addition + closing));
    }
    Ok(output)
}

/// Materialize a deployment copy, never patch the official installation or tracked MOD.
pub fn prepare(repo: &Repository, game: &Path, output: &Path) -> Result<()> {
    prepare_local_dlc_mod(repo, game, output, false)
}

fn installed_patches(game: &Path) -> Result<Vec<DatabasePatch>> {
    let mut patches = Vec::new();
    for dlc in DLCS {
        let dir = game.join("DLC").join(dlc.name);
        let path = dir.join(format!("{}.modinfo", dlc.name));
        match fs::metadata(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e).with_context(|| format!("inspect {}", path.display())),
            Ok(_) => {}
        }
        let manifest = read_utf8(&path)?;
        patches.push(DatabasePatch {
            id: format!("JAC_Banseiki_{}", dlc.name),
            dlc_id: dlc.id,
            mod_in_use: None,
            sql: compile_files(&dir, &dlc_sources(&manifest, dlc)?)?,
        });
        if dlc.name == "Babylon" {
            let doc = Document::parse(&manifest)?;
            let cross_action = doc
                .descendants()
                .find(|n| {
                    n.has_tag_name("UpdateDatabase")
                        && n.attribute("id") == Some("BabylonGameplay_GranColombia_Maya")
                })
                .context("Babylon cross-DLC action")?;
            ensure!(
                cross_action.attribute("criteria") == Some("Babylon_GranColombia_Maya"),
                "unexpected cross-DLC action criterion"
            );
            let condition = doc
                .descendants()
                .find(|n| {
                    n.has_tag_name("Criteria")
                        && n.attribute("id") == Some("Babylon_GranColombia_Maya")
                })
                .context("Babylon cross-DLC criterion")?;
            ensure!(
                condition.attribute("any").is_none(),
                "unexpected cross-DLC OR condition"
            );
            let terms: BTreeMap<_, _> = condition
                .children()
                .filter(Node::is_element)
                .map(|n| (n.tag_name().name(), n.text().unwrap_or("")))
                .collect();
            ensure!(
                terms
                    == BTreeMap::from([
                        (
                            "RuleSetInUse",
                            "RULESET_STANDARD,RULESET_EXPANSION_1,RULESET_EXPANSION_2"
                        ),
                        ("ModInUse", MAYA_ID)
                    ]),
                "unexpected cross-DLC criterion"
            );
            let files = component_sources(&manifest, dlc, "BabylonGameplay_GranColombia_Maya", 1)?;
            patches.push(DatabasePatch {
                id: "JAC_Banseiki_Babylon_GranColombia_Maya".into(),
                dlc_id: dlc.id,
                mod_in_use: Some(MAYA_ID),
                sql: compile_files(&dir, &files)?,
            });
        }
    }
    Ok(patches)
}

fn is_link(metadata: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        metadata.file_type().is_symlink()
    }
}

fn regular_paths(root: &Path) -> Result<Vec<PathBuf>> {
    ensure!(
        !is_link(&fs::symlink_metadata(root)?),
        "output/source root is a link or junction"
    );
    let mut files = Vec::new();
    for entry in walkdir::WalkDir::new(root).follow_links(false) {
        let entry = entry?;
        ensure!(
            !is_link(&fs::symlink_metadata(entry.path())?),
            "link or junction: {}",
            entry.path().display()
        );
        if entry.file_type().is_file() {
            files.push(entry.path().strip_prefix(root)?.to_owned());
        }
    }
    Ok(files)
}

fn regular_tree(root: &Path) -> Result<BTreeMap<PathBuf, Vec<u8>>> {
    regular_paths(root)?
        .into_iter()
        .map(|path| Ok((path.clone(), fs::read(root.join(path))?)))
        .collect()
}

fn owner_bytes(files: &BTreeMap<PathBuf, Vec<u8>>) -> Result<Vec<u8>> {
    let paths: Vec<_> = files
        .keys()
        .map(|path| path.to_string_lossy().replace('\\', "/"))
        .collect();
    Ok(format!("{}\n", serde_json::to_string_pretty(&serde_json::json!({
        "format_version": 1, "generated_by": GENERATOR, "source_mod_id": SOURCE_MOD_ID, "files": paths,
    }))?).into_bytes())
}

fn destination(output: &Path) -> Result<PathBuf> {
    ensure!(output.is_absolute(), "output must be absolute");
    ensure!(
        !output.components().any(|c| matches!(
            c,
            std::path::Component::ParentDir | std::path::Component::CurDir
        )),
        "output must not contain . or .."
    );
    let mut suffix = vec![output.file_name().context("output folder name")?.to_owned()];
    let mut parent = output.parent().context("output parent")?;
    loop {
        match fs::symlink_metadata(parent) {
            Ok(_) => break,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                suffix.push(
                    parent
                        .file_name()
                        .context("existing output ancestor")?
                        .to_owned(),
                );
                parent = parent.parent().context("output ancestor")?;
            }
            Err(e) => return Err(e.into()),
        }
    }
    let mut resolved = parent.canonicalize()?;
    for name in suffix.into_iter().rev() {
        resolved.push(name);
    }
    Ok(resolved)
}

/// Fresh refuses all existing outputs. Refresh validates ownership, MOD id and
/// every output entry before writing; no directory/file deletion or permission changes.
pub fn prepare_local_dlc_mod(
    repo: &Repository,
    game: &Path,
    output: &Path,
    refresh: bool,
) -> Result<()> {
    let source = repo.path(MOD_DIR).canonicalize()?;
    let output = destination(output)?;
    ensure!(
        !output.starts_with(game.canonicalize()?),
        "output is inside the official installation"
    );
    ensure!(
        !output.starts_with(&source),
        "output is inside the source MOD"
    );
    let mut files = regular_tree(&source)?;
    ensure!(
        !files.contains_key(Path::new(OWNER_FILE)),
        "source is a generated MOD"
    );
    let source_info =
        std::str::from_utf8(files.get(Path::new(MODINFO)).context("source modinfo")?)?
            .trim_start_matches('\u{feff}')
            .replace("\r\n", "\n");
    let source_doc = Document::parse(&source_info)?;
    for declared in source_doc
        .descendants()
        .filter(|n| n.has_tag_name("File") && n.parent().is_some_and(|p| p.has_tag_name("Files")))
    {
        let path = Path::new(declared.text().context("source declared file")?);
        ensure!(
            files.contains_key(path),
            "source declared file is missing: {}",
            path.display()
        );
    }
    let patches = installed_patches(game)?;
    let info = manifest_with_patches(&source_info, &patches)?;
    files.insert(MODINFO.into(), info.into_bytes());
    // Stable inactive placeholders avoid deletion when an installed DLC disappears.
    for name in ["Ethiopia", "Babylon", "Babylon_GranColombia_Maya"] {
        files.insert(
            PathBuf::from(format!("Data/JAC_Banseiki_{name}.sql")),
            b"-- DLC not installed; no action references this file.\n".to_vec(),
        );
    }
    for patch in patches {
        files.insert(patch.file().into(), patch.sql.into_bytes());
    }
    let exists = match fs::symlink_metadata(&output) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => false,
        Err(e) => return Err(e.into()),
        Ok(_) => {
            ensure!(refresh, "output already exists; --refresh is required");
            let paths = regular_paths(&output)?;
            ensure!(
                paths.contains(&PathBuf::from(OWNER_FILE)),
                "output is not an owned generated MOD"
            );
            let owner: serde_json::Value =
                serde_json::from_slice(&fs::read(output.join(OWNER_FILE))?)?;
            ensure!(
                owner["format_version"] == 1
                    && owner["generated_by"] == GENERATOR
                    && owner["source_mod_id"] == SOURCE_MOD_ID,
                "output ownership mismatch"
            );
            let owned = owner["files"].as_array().context("owned file inventory")?;
            let owned: std::collections::BTreeSet<PathBuf> = owned
                .iter()
                .map(|value| value.as_str().map(PathBuf::from).context("owned file name"))
                .collect::<Result<_>>()?;
            ensure!(
                paths
                    .iter()
                    .all(|path| path == Path::new(OWNER_FILE) || owned.contains(path)),
                "output contains an unowned file"
            );
            let old_info = fs::read_to_string(output.join(MODINFO)).context("output modinfo")?;
            let old_doc = Document::parse(&old_info)?;
            ensure!(
                old_doc
                    .root_element()
                    .attribute("id")
                    .is_some_and(|id| id.eq_ignore_ascii_case(SOURCE_MOD_ID)),
                "output MOD id mismatch"
            );
            // Obsolete source files stay unlisted; retain ownership
            // of these remnants so subsequent refreshes also remain safe.
            for path in paths {
                if path != Path::new(OWNER_FILE) && !files.contains_key(&path) {
                    files.insert(path.clone(), fs::read(output.join(&path))?);
                }
            }
            true
        }
    };
    let owner = owner_bytes(&files)?;
    if !exists {
        fs::create_dir_all(output.parent().context("output parent")?)?;
        fs::create_dir(&output)?; // fresh never adopts a concurrently created directory
    }
    for (relative, bytes) in files {
        let dest = output.join(relative);
        fs::create_dir_all(dest.parent().context("file parent")?)?;
        fs::write(dest, bytes)?;
    }
    fs::write(output.join(OWNER_FILE), owner)?;
    Ok(())
}
