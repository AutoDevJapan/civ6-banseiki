//! Prepare a local Banseiki compatibility MOD from the user's installed Ethiopia DLC.
//! Official XML/SQL is neither copied into the repository nor shipped in the MOD sources.
use crate::{read_utf8, sql_quote, Repository};
use anyhow::{bail, ensure, Context, Result};
use roxmltree::{Document, Node};
use std::{collections::BTreeMap, path::Path};

pub const MOD_DIR: &str = "poc/JAC_PoC_BanseikiRuleset";
pub const MODINFO: &str = "JAC_PoC_BanseikiRuleset.modinfo";
pub const DLC_ID: &str = "1B394FE9-23DC-4868-8F0A-5220CB8FB427";
pub const SQL_FILE: &str = "Data/JAC_Banseiki_Ethiopia.sql";
const COMPONENTS: [&str; 2] = ["EthiopiaGameplay", "EthiopiaGameplay_XP2"];

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
    let doc = Document::parse(manifest.trim_start_matches('\u{feff}'))?;
    ensure!(
        doc.root_element()
            .attribute("id")
            .is_some_and(|id| id.eq_ignore_ascii_case(DLC_ID)),
        "not the Ethiopia DLC"
    );
    let mut paths = Vec::new();
    for component in COMPONENTS {
        let action = doc
            .descendants()
            .find(|n| n.has_tag_name("UpdateDatabase") && n.attribute("id") == Some(component))
            .with_context(|| format!("missing {component}"))?;
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
    }
    ensure!(paths.len() == 10, "expected eight base and two XP2 files");
    Ok(paths)
}

pub fn gameplay_sql(game: &Path) -> Result<String> {
    let dlc = game.join("DLC/Ethiopia");
    let files = sources(&read_utf8(&dlc.join("Ethiopia.modinfo"))?)?;
    let mut out = "-- Locally generated from installed Ethiopia DLC; do not redistribute.\n\
        -- Official components finish at 0. Repair base rows, then replay XP2 updates at 1.\n"
        .to_owned();
    for file in files {
        out.push_str(&format!("-- {file}\n"));
        out.push_str(
            &xml_sql(&read_utf8(&dlc.join(&file))?).with_context(|| format!("convert {file}"))?,
        );
    }
    Ok(out)
}

/// Modify only the prepared copy; keep the original source MOD usable without DLC.
pub fn modinfo(source: &str) -> Result<String> {
    let doc = Document::parse(source)?;
    ensure!(
        !doc.descendants()
            .any(|n| n.attribute("id") == Some("JAC_Banseiki_Ethiopia")),
        "MOD already prepared"
    );
    let criteria = format!(
        "\t\t<Criteria id=\"JAC_Banseiki_Ethiopia\">\n\
        \t\t\t<RuleSetInUse>RULESET_JAC_BANSEIKI</RuleSetInUse>\n\
        \t\t\t<GameCoreInUse>Expansion2</GameCoreInUse>\n\
        \t\t\t<ModIsEnabled>{DLC_ID}</ModIsEnabled>\n\
        \t\t</Criteria>\n"
    );
    let action = format!(
        "\t\t<UpdateDatabase id=\"JAC_Banseiki_Ethiopia\" criteria=\"JAC_Banseiki_Ethiopia\">\n\
        \t\t\t<Properties><LoadOrder>1</LoadOrder></Properties>\n\
        \t\t\t<File>{SQL_FILE}</File>\n\
        \t\t</UpdateDatabase>\n"
    );
    let mut output = source.to_owned();
    for (closing, addition) in [
        ("\t</ActionCriteria>", criteria),
        ("\t</InGameActions>", action),
        ("\t</Files>", format!("\t\t<File>{SQL_FILE}</File>\n")),
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
    let source = repo.path(MOD_DIR).canonicalize()?;
    ensure!(output.is_absolute(), "output must be absolute");
    let output = output
        .parent()
        .context("output parent")?
        .canonicalize()?
        .join(output.file_name().context("output folder name")?);
    ensure!(
        !output.starts_with(game.canonicalize()?),
        "output is inside the official installation"
    );
    ensure!(
        !output.starts_with(&source),
        "output is inside the source MOD"
    );
    ensure!(
        !output.exists(),
        "output already exists; use a new destination"
    );
    let sql = gameplay_sql(game)?;
    let info = modinfo(&read_utf8(&source.join(MODINFO))?)?;
    let entries: Vec<_> = walkdir::WalkDir::new(&source)
        .into_iter()
        .collect::<std::result::Result<_, _>>()?;
    ensure!(
        !entries.iter().any(|e| e.file_type().is_symlink()),
        "source MOD contains a link"
    );
    for entry in entries {
        let relative = entry.path().strip_prefix(&source)?;
        let dest = output.join(relative);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&dest)?;
        } else {
            std::fs::copy(entry.path(), &dest)?;
        }
    }
    std::fs::write(output.join(SQL_FILE), sql)?;
    std::fs::write(output.join(MODINFO), info)?;
    Ok(())
}
