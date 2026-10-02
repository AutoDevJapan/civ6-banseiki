//! Installed MOD identity, file freshness, and per-MOD atlas texture checks.

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use serde_json::Value;
use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, Metadata};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

fn normalize_mod_id(id: &str) -> Result<String, String> {
    let bytes = id.as_bytes();
    if bytes.len() != 36
        || !bytes.iter().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                *byte == b'-'
            } else {
                byte.is_ascii_hexdigit()
            }
        })
    {
        return Err(format!("Missing or invalid MOD GUID: {id:?}"));
    }
    Ok(id.to_ascii_lowercase())
}

fn scenario_mod_ids(scenario: &Value) -> Result<BTreeSet<String>, String> {
    let steps = scenario
        .get("steps")
        .and_then(Value::as_array)
        .ok_or("Scenario steps must be an array")?;
    let mut ids = BTreeSet::new();
    for (index, step) in steps.iter().enumerate() {
        if step.get("action").and_then(Value::as_str) != Some("enable_mods") {
            continue;
        }
        let enabled = step
            .get("ids")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("enable_mods step {index} must contain an ids array"))?;
        for id in enabled {
            let id = id
                .as_str()
                .ok_or_else(|| format!("MOD ID in step {index} must be a string"))?;
            ids.insert(normalize_mod_id(id)?);
        }
    }
    Ok(ids)
}

fn read_dir_sorted(path: &Path) -> Result<Vec<PathBuf>, String> {
    let entries = fs::read_dir(path)
        .map_err(|error| format!("Cannot read directory {}: {error}", path.display()))?;
    let mut paths = Vec::new();
    for entry in entries {
        paths.push(
            entry
                .map_err(|error| format!("Cannot enumerate directory {}: {error}", path.display()))?
                .path(),
        );
    }
    paths.sort();
    Ok(paths)
}

fn has_extension(path: &Path, extension: &str) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case(extension))
}

#[derive(Default, Debug)]
struct XmlSummary {
    root: Vec<u8>,
    mod_id: Option<String>,
    atlas_filenames: Vec<String>,
}

fn valid_xml_chars(text: &str) -> bool {
    text.chars().all(|ch| matches!(ch,
        '\t' | '\n' | '\r' | '\u{20}'..='\u{d7ff}' | '\u{e000}'..='\u{fffd}' | '\u{10000}'..='\u{10ffff}'
    ))
}

fn summarize_element(
    element: &BytesStart<'_>,
    parents: &[Vec<u8>],
    decoder: quick_xml::encoding::Decoder,
    summary: &mut XmlSummary,
) -> Result<(), String> {
    let is_mod_root = parents.is_empty() && element.name().as_ref() == b"Mod";
    // ElementTree's ./IconTextureAtlases/Row selects only direct children of
    // the document root, not a Row in an unrelated nested table or namespace.
    let is_atlas_row = parents.len() == 2
        && parents[1].as_slice() == b"IconTextureAtlases"
        && element.name().as_ref() == b"Row";
    for attribute in element.attributes() {
        let attribute = attribute.map_err(|error| error.to_string())?;
        let value = attribute
            .decode_and_unescape_value(decoder)
            .map_err(|error| error.to_string())?;
        if !valid_xml_chars(&value) {
            return Err("Invalid XML character in attribute".into());
        }
        if is_mod_root && attribute.key.as_ref() == b"id" {
            summary.mod_id = Some(value.into_owned());
        } else if is_atlas_row && attribute.key.as_ref() == b"Filename" && !value.is_empty() {
            summary.atlas_filenames.push(value.into_owned());
        }
    }
    Ok(())
}

fn xml_text(bytes: &[u8]) -> Result<Cow<'_, str>, String> {
    let utf16 = if let Some(body) = bytes.strip_prefix(&[0xff, 0xfe]) {
        Some((body, true))
    } else if let Some(body) = bytes.strip_prefix(&[0xfe, 0xff]) {
        Some((body, false))
    } else if bytes.starts_with(&[b'<', 0]) {
        Some((bytes, true))
    } else if bytes.starts_with(&[0, b'<']) {
        Some((bytes, false))
    } else {
        None
    };
    if let Some((body, little_endian)) = utf16 {
        if body.len() % 2 != 0 {
            return Err("Truncated UTF-16 XML".into());
        }
        let words: Vec<u16> = body
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| {
                if little_endian {
                    u16::from_le_bytes([pair[0], pair[1]])
                } else {
                    u16::from_be_bytes([pair[0], pair[1]])
                }
            })
            .collect();
        return String::from_utf16(&words)
            .map(Cow::Owned)
            .map_err(|error| error.to_string());
    }
    let body = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes);
    std::str::from_utf8(body)
        .map(Cow::Borrowed)
        .map_err(|error| error.to_string())
}

fn parse_xml(bytes: &[u8]) -> Result<XmlSummary, String> {
    // ElementTree also reads UTF-16 MOD/XML files. Decode them first: quick-xml
    // without its optional encoding dependency scans ASCII-compatible bytes only.
    let text = xml_text(bytes)?;
    // from_str pins the already-decoded UTF-8 encoding, even if a retained XML
    // declaration names UTF-16 or the caller enables quick-xml's encoding feature.
    let mut reader = Reader::from_str(&text);
    reader.config_mut().check_comments = true;
    reader.config_mut().expand_empty_elements = true;
    let mut buffer = Vec::new();
    let mut parents: Vec<Vec<u8>> = Vec::new();
    let mut summary = XmlSummary::default();
    let mut root_seen = false;
    loop {
        match reader
            .read_event_into(&mut buffer)
            .map_err(|error| error.to_string())?
        {
            Event::Start(element) => {
                if parents.is_empty() {
                    if root_seen {
                        return Err("Multiple XML roots".into());
                    }
                    root_seen = true;
                    summary.root = element.name().as_ref().to_vec();
                }
                summarize_element(&element, &parents, reader.decoder(), &mut summary)?;
                // Empty elements arrive as Start followed by End, so both forms
                // use the same ancestor path and full-document validation.
                parents.push(element.name().as_ref().to_vec());
            }
            Event::End(element) if parents.pop().as_deref() != Some(element.name().as_ref()) => {
                return Err("Unmatched XML end element".into());
            }
            Event::Text(text) => {
                let text = text.unescape().map_err(|error| error.to_string())?;
                if !valid_xml_chars(&text)
                    || (parents.is_empty()
                        && text
                            .chars()
                            .any(|ch| !matches!(ch, ' ' | '\t' | '\r' | '\n')))
                {
                    return Err("Invalid XML text".into());
                }
            }
            Event::CData(text) => {
                let text = text.decode().map_err(|error| error.to_string())?;
                if parents.is_empty() || !valid_xml_chars(&text) {
                    return Err("Invalid XML CDATA".into());
                }
            }
            Event::Decl(declaration) => {
                if root_seen {
                    return Err("XML declaration after the root element".into());
                }
                declaration.version().map_err(|error| error.to_string())?;
            }
            Event::Eof => {
                if !root_seen || !parents.is_empty() {
                    return Err("Missing or unclosed XML root".into());
                }
                return Ok(summary);
            }
            _ => {}
        }
        buffer.clear();
    }
}

fn mod_id_from_document(summary: &XmlSummary) -> Result<Option<String>, String> {
    if summary.root.as_slice() != b"Mod" {
        return Ok(None);
    }
    let id = summary
        .mod_id
        .as_deref()
        .ok_or("modinfo Mod element has no GUID")?;
    normalize_mod_id(id).map(Some)
}

fn record_enabled_mod(
    found: &mut BTreeMap<String, PathBuf>,
    wanted: &BTreeSet<String>,
    id: String,
    folder: &Path,
) -> Result<(), String> {
    if !wanted.contains(&id) {
        return Ok(());
    }
    if let Some(previous) = found.get(&id) {
        return Err(format!(
            "Duplicate installed mod ID {id}: {} and {}",
            previous.display(),
            folder.display()
        ));
    }
    found.insert(id, folder.to_path_buf());
    Ok(())
}

fn resolve_enabled_mods(
    wanted: &BTreeSet<String>,
    found: &BTreeMap<String, PathBuf>,
    mods_dir: &Path,
) -> Result<Vec<PathBuf>, String> {
    let missing: Vec<&str> = wanted
        .iter()
        .filter(|id| !found.contains_key(*id))
        .map(String::as_str)
        .collect();
    if !missing.is_empty() {
        return Err(format!(
            "Scenario mod is not installed under {}: {}",
            mods_dir.display(),
            missing.join(", ")
        ));
    }
    Ok(wanted.iter().map(|id| found[id].clone()).collect())
}

pub fn enabled_mod_folders(scenario: &Value, mods_dir: &Path) -> Result<Vec<PathBuf>, String> {
    let wanted = scenario_mod_ids(scenario)?;
    if wanted.is_empty() {
        return Ok(Vec::new());
    }
    let mut found = BTreeMap::new();
    for entry in read_dir_sorted(mods_dir)? {
        // Follow the Mods junction and its top-level MOD folder junctions. The
        // returned absolute targets are the roots of the subsequent bounded walk.
        let metadata = fs::metadata(&entry)
            .map_err(|error| format!("Cannot inspect MOD entry {}: {error}", entry.display()))?;
        if !metadata.is_dir() {
            continue;
        }
        let folder = fs::canonicalize(&entry)
            .map_err(|error| format!("Cannot resolve MOD folder {}: {error}", entry.display()))?;
        for info in read_dir_sorted(&folder)? {
            if !has_extension(&info, "modinfo") {
                continue;
            }
            let bytes = fs::read(&info)
                .map_err(|error| format!("Cannot read modinfo {}: {error}", info.display()))?;
            // Invalid XML is ignored just as by the old ElementTree scanner. It
            // cannot satisfy a requested GUID, which still produces a missing error.
            let Ok(summary) = parse_xml(&bytes) else {
                continue;
            };
            let id = mod_id_from_document(&summary)
                .map_err(|error| format!("{}: {error}", info.display()))?;
            if let Some(id) = id {
                record_enabled_mod(&mut found, &wanted, id, &folder)?;
            }
        }
    }
    resolve_enabled_mods(&wanted, &found, mods_dir)
}

/// Official DLC is catalogued under the running game's install, while only
/// user MODs require per-folder freshness and custom atlas checks.
pub fn enabled_custom_mod_folders(
    scenario: &Value,
    mods_dir: &Path,
    dlc_dir: &Path,
) -> Result<Vec<PathBuf>, String> {
    let wanted = scenario_mod_ids(scenario)?;
    let mut official = BTreeMap::new();
    for info in mod_files(dlc_dir)? {
        if !has_extension(&info, "modinfo") {
            continue;
        }
        let bytes = fs::read(&info)
            .map_err(|e| format!("Cannot read official DLC {}: {e}", info.display()))?;
        let Ok(summary) = parse_xml(&bytes) else {
            continue;
        };
        if let Some(id) = mod_id_from_document(&summary)?
            && wanted.contains(&id)
            && official.insert(id.clone(), info.clone()).is_some()
        {
            return Err(format!("duplicate official DLC GUID {id}"));
        }
    }
    let mut custom = scenario.clone();
    for step in custom["steps"]
        .as_array_mut()
        .ok_or("scenario steps missing")?
    {
        if step["action"] == "enable_mods" {
            step["ids"]
                .as_array_mut()
                .ok_or("enable_mods IDs missing")?
                .retain(|v| {
                    v.as_str()
                        .is_none_or(|s| !official.contains_key(&s.to_ascii_lowercase()))
                });
        }
    }
    enabled_mod_folders(&custom, mods_dir)
}

fn directory_link_flags(is_symlink: bool, attributes: u32) -> bool {
    is_symlink || attributes & 0x400 != 0
}

fn directory_is_link(metadata: &Metadata) -> bool {
    #[cfg(windows)]
    let attributes = {
        use std::os::windows::fs::MetadataExt;
        // FILE_ATTRIBUTE_REPARSE_POINT also detects mount-point junctions;
        // checking FileType::is_symlink alone is insufficient on Windows.
        metadata.file_attributes()
    };
    #[cfg(not(windows))]
    let attributes = 0;
    directory_link_flags(metadata.file_type().is_symlink(), attributes)
}

fn mod_files(folder: &Path) -> Result<Vec<PathBuf>, String> {
    let root = fs::canonicalize(folder)
        .map_err(|error| format!("Cannot resolve MOD folder {}: {error}", folder.display()))?;
    if !fs::metadata(&root)
        .map_err(|error| format!("Cannot inspect MOD folder {}: {error}", root.display()))?
        .is_dir()
    {
        return Err(format!("MOD folder is not a directory: {}", root.display()));
    }
    let mut pending = vec![root.clone()];
    let mut visited = BTreeSet::new();
    let mut files = Vec::new();
    while let Some(directory) = pending.pop() {
        let metadata = fs::symlink_metadata(&directory).map_err(|error| {
            format!(
                "Cannot inspect MOD directory {}: {error}",
                directory.display()
            )
        })?;
        if directory_is_link(&metadata) {
            continue;
        }
        let resolved = fs::canonicalize(&directory).map_err(|error| {
            format!(
                "Cannot resolve MOD directory {}: {error}",
                directory.display()
            )
        })?;
        if !resolved.starts_with(&root) {
            return Err(format!(
                "MOD directory changed to an external target: {}",
                directory.display()
            ));
        }
        if !visited.insert(resolved.clone()) {
            continue;
        }
        for path in read_dir_sorted(&resolved)? {
            let metadata = fs::symlink_metadata(&path)
                .map_err(|error| format!("Cannot inspect MOD file {}: {error}", path.display()))?;
            if metadata.is_dir() {
                if !directory_is_link(&metadata) {
                    pending.push(path);
                }
            } else if metadata.file_type().is_symlink() {
                // File symlinks have no recursive walk. As in os.walk/stat,
                // their target's mtime/content is used; directory links are skipped.
                let target = fs::metadata(&path).map_err(|error| {
                    format!("Cannot inspect MOD link {}: {error}", path.display())
                })?;
                if target.is_file() {
                    files.push(path);
                }
            } else if metadata.is_file() {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

fn consider_newest(newest: &mut Option<(PathBuf, SystemTime)>, path: &Path, time: SystemTime) {
    let replace = match newest {
        None => true,
        Some((previous, modified)) => {
            time > *modified || (time == *modified && path < previous.as_path())
        }
    };
    if replace {
        *newest = Some((path.to_path_buf(), time));
    }
}

pub fn newest_mod_time(folders: &[PathBuf]) -> Result<Option<(PathBuf, SystemTime)>, String> {
    let mut newest = None;
    for folder in folders {
        for path in mod_files(folder)? {
            let modified = fs::metadata(&path)
                .and_then(|metadata| metadata.modified())
                .map_err(|error| {
                    format!("Cannot read MOD file mtime {}: {error}", path.display())
                })?;
            consider_newest(&mut newest, &path, modified);
        }
    }
    if !folders.is_empty() && newest.is_none() {
        return Err("Enabled mod folders contain no files".into());
    }
    Ok(newest)
}

fn texture_stem(filename: &str) -> Option<String> {
    // XML may contain Windows separators even when pure tests run on another OS.
    let basename = filename.rsplit(['/', '\\']).next()?;
    Path::new(basename)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .map(str::to_owned)
}

fn atlas_names(summary: &XmlSummary) -> Result<BTreeSet<String>, String> {
    summary
        .atlas_filenames
        .iter()
        .map(|filename| {
            texture_stem(filename)
                .ok_or_else(|| format!("Invalid IconTextureAtlases Filename: {filename:?}"))
        })
        .collect()
}

fn remove_loose_dds(names: &mut BTreeSet<String>, files: &[PathBuf]) {
    for path in files.iter().filter(|path| has_extension(path, "dds")) {
        if let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) {
            names.remove(stem);
        }
    }
}

fn remove_packaged_names(mut reader: impl Read, names: &mut BTreeSet<String>) -> io::Result<()> {
    // BLPs can be large. Carry enough overlap to find a name split across reads
    // without loading every installed package into memory at once.
    let mut chunk = [0u8; 65_536];
    let mut window = Vec::new();
    while !names.is_empty() {
        let read = match reader.read(&mut chunk) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            result => result?,
        };
        if read == 0 {
            break;
        }
        window.extend_from_slice(&chunk[..read]);
        names.retain(|name| {
            !window
                .windows(name.len())
                .any(|bytes| bytes == name.as_bytes())
        });
        let keep = names
            .iter()
            .map(String::len)
            .max()
            .unwrap_or(1)
            .saturating_sub(1);
        window.drain(..window.len().saturating_sub(keep));
    }
    Ok(())
}

pub fn missing_atlas_textures(folders: &[PathBuf]) -> Result<Vec<String>, String> {
    let mut missing = Vec::new();
    for folder in folders {
        let files = mod_files(folder)?;
        if files.is_empty() {
            return Err(format!(
                "Enabled MOD folder contains no files: {}",
                folder.display()
            ));
        }
        let mut names = BTreeSet::new();
        for path in files.iter().filter(|path| has_extension(path, "xml")) {
            let bytes = fs::read(path)
                .map_err(|error| format!("Cannot read MOD XML {}: {error}", path.display()))?;
            // Non-database/invalid XML was also ignored by the old scanner. A
            // partially parsed malformed document must not contribute atlas rows.
            if let Ok(summary) = parse_xml(&bytes) {
                names.extend(
                    atlas_names(&summary)
                        .map_err(|error| format!("{}: {error}", path.display()))?,
                );
            }
        }
        remove_loose_dds(&mut names, &files);
        for path in files.iter().filter(|path| has_extension(path, "blp")) {
            if names.is_empty() {
                break;
            }
            let package = fs::File::open(path)
                .map_err(|error| format!("Cannot open MOD BLP {}: {error}", path.display()))?;
            remove_packaged_names(package, &mut names)
                .map_err(|error| format!("Cannot read MOD BLP {}: {error}", path.display()))?;
        }
        let label = folder
            .file_name()
            .unwrap_or(folder.as_os_str())
            .to_string_lossy();
        missing.extend(names.into_iter().map(|name| format!("{label}: {name}")));
    }
    Ok(missing)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::io::Cursor;
    use std::time::{Duration, UNIX_EPOCH};

    const MOD_A: &str = "ba3a1c58-9287-4654-a22f-85f9e6841e9b";
    const MOD_B: &str = "bd7a82c5-6b38-4b69-a7f5-8dc9af534249";

    #[test]
    fn official_dlc_is_found_in_install_and_custom_mods_stay_mandatory() {
        let temp = std::env::temp_dir();
        let root = temp.join(format!(
            "native-dlc-test-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        let mods = root.join("Mods");
        let dlc = root.join("DLC");
        let custom = mods.join("Custom");
        let official = dlc.join("Babylon");
        fs::create_dir_all(&custom).unwrap();
        fs::create_dir_all(&official).unwrap();
        let official_info = official.join("Babylon.modinfo");
        fs::write(&official_info, format!("<Mod id=\"{MOD_A}\"/>")).unwrap();
        fs::write(
            custom.join("Custom.modinfo"),
            format!("<Mod id=\"{MOD_B}\"/>"),
        )
        .unwrap();
        let scenario = json!({"steps":[{"action":"enable_mods","ids":[MOD_A,MOD_B]}]});
        assert_eq!(
            enabled_custom_mod_folders(&scenario, &mods, &dlc).unwrap(),
            vec![fs::canonicalize(&custom).unwrap()]
        );
        fs::remove_file(&official_info).unwrap();
        assert!(
            enabled_custom_mod_folders(&scenario, &mods, &dlc)
                .unwrap_err()
                .contains(MOD_A)
        );
        fs::write(&official_info, format!("<Mod id=\"{MOD_A}\"/>")).unwrap();
        fs::remove_file(custom.join("Custom.modinfo")).unwrap();
        assert!(
            enabled_custom_mod_folders(&scenario, &mods, &dlc)
                .unwrap_err()
                .contains(MOD_B)
        );
        let resolved = fs::canonicalize(&root).unwrap();
        let temp = fs::canonicalize(temp).unwrap();
        assert!(resolved.starts_with(&temp) && resolved != temp);
        fs::remove_dir_all(resolved).unwrap();
    }

    #[test]
    fn enabled_ids_are_case_insensitive_deduplicated_and_sorted() {
        let scenario = json!({"steps": [
            {"action": "pause"},
            {"action": "enable_mods", "ids": [MOD_B, MOD_A.to_uppercase()]},
            {"action": "enable_mods", "ids": [MOD_A]}
        ]});
        assert_eq!(
            scenario_mod_ids(&scenario).unwrap(),
            BTreeSet::from([MOD_A.into(), MOD_B.into()])
        );
        assert!(scenario_mod_ids(&json!({"steps": []})).unwrap().is_empty());
    }

    #[test]
    fn malformed_enabled_ids_are_errors_not_an_empty_mod_set() {
        for scenario in [
            json!({}),
            json!({"steps": [{"action": "enable_mods"}]}),
            json!({"steps": [{"action": "enable_mods", "ids": [null]}]}),
            json!({"steps": [{"action": "enable_mods", "ids": [""]}]}),
            json!({"steps": [{"action": "enable_mods", "ids": ["not-a-guid"]}]}),
        ] {
            assert!(scenario_mod_ids(&scenario).is_err(), "{scenario}");
        }
    }

    #[test]
    fn modinfo_requires_a_guid_on_the_actual_mod_root() {
        let document =
            parse_xml(format!("<Mod id=\"{}\" version=\"1\"/>", MOD_A.to_uppercase()).as_bytes())
                .unwrap();
        assert_eq!(mod_id_from_document(&document).unwrap(), Some(MOD_A.into()));
        let unrelated = parse_xml(b"<GameInfo><Mod id=\"unrelated\"/></GameInfo>").unwrap();
        assert_eq!(mod_id_from_document(&unrelated).unwrap(), None);
        for xml in [
            b"<Mod/>".as_slice(),
            b"<Mod id=\"\"/>",
            b"<Mod id=\"invalid\"/>",
        ] {
            assert!(mod_id_from_document(&parse_xml(xml).unwrap()).is_err());
        }
    }

    #[test]
    fn duplicate_and_missing_installed_ids_are_rejected_including_aliases() {
        let wanted = BTreeSet::from([MOD_A.into(), MOD_B.into()]);
        let mut found = BTreeMap::new();
        let folder = Path::new("resolved-mod-target");
        record_enabled_mod(&mut found, &wanted, MOD_A.into(), folder).unwrap();
        assert!(
            record_enabled_mod(&mut found, &wanted, MOD_A.into(), folder)
                .unwrap_err()
                .contains("Duplicate")
        );
        assert!(
            resolve_enabled_mods(&wanted, &found, Path::new("Mods"))
                .unwrap_err()
                .contains(MOD_B)
        );
        record_enabled_mod(
            &mut found,
            &wanted,
            MOD_B.into(),
            Path::new("second-target"),
        )
        .unwrap();
        assert_eq!(
            resolve_enabled_mods(&wanted, &found, Path::new("Mods")).unwrap(),
            vec![folder.to_path_buf(), PathBuf::from("second-target")]
        );
    }

    #[test]
    fn atlas_rows_use_the_same_direct_path_and_xml_unescaping_as_the_old_scanner() {
        let document = parse_xml(
            br#"<GameInfo>
            <IconTextureAtlases><Row Filename="Textures\JAC_Crests36.dds"/>
                <Row Filename="Art/A&amp;B.dds"/><Row Name="no filename"/></IconTextureAtlases>
            <Other><IconTextureAtlases><Row Filename="Unrelated.dds"/></IconTextureAtlases></Other>
            <LocalizedText><Row Filename="Text.dds"/></LocalizedText>
        </GameInfo>"#,
        )
        .unwrap();
        assert_eq!(
            atlas_names(&document).unwrap(),
            BTreeSet::from(["A&B".into(), "JAC_Crests36".into()])
        );
    }

    #[test]
    fn malformed_xml_cannot_contribute_partial_atlas_or_mod_data() {
        for xml in [
            b"<GameInfo><IconTextureAtlases><Row Filename=\"A.dds\"/></IconTextureAtlases>"
                .as_slice(),
            b"<GameInfo><IconTextureAtlases></GameInfo>",
            b"<GameInfo/><GameInfo/>",
            b"<GameInfo><Row Filename=\"&unknown;\"/></GameInfo>",
            b"<Mod id=\"one\" id=\"two\"/>",
            b"<GameInfo><!-- broken -- comment --></GameInfo>",
        ] {
            assert!(parse_xml(xml).is_err(), "{}", String::from_utf8_lossy(xml));
        }
    }

    #[test]
    fn utf16_atlas_xml_is_checked_in_both_byte_orders() {
        let xml = "<?xml version=\"1.0\" encoding=\"UTF-16\"?><GameInfo><IconTextureAtlases><Row Filename=\"Crests36.dds\"/></IconTextureAtlases></GameInfo>";
        for little_endian in [true, false] {
            let mut bytes = if little_endian {
                vec![0xff, 0xfe]
            } else {
                vec![0xfe, 0xff]
            };
            for word in xml.encode_utf16() {
                bytes.extend_from_slice(&if little_endian {
                    word.to_le_bytes()
                } else {
                    word.to_be_bytes()
                });
            }
            assert_eq!(
                atlas_names(&parse_xml(&bytes).unwrap()).unwrap(),
                BTreeSet::from(["Crests36".into()])
            );
            assert_eq!(
                atlas_names(&parse_xml(&bytes[2..]).unwrap()).unwrap(),
                BTreeSet::from(["Crests36".into()])
            );
            bytes.pop();
            assert!(parse_xml(&bytes).is_err());
        }
    }

    #[test]
    fn directory_junctions_and_symlinks_are_not_recursed() {
        assert!(!directory_link_flags(false, 0x10)); // ordinary directory
        assert!(!directory_link_flags(false, 0x12)); // hidden directory
        assert!(directory_link_flags(false, 0x410)); // mount-point junction
        assert!(directory_link_flags(true, 0x10)); // directory symlink
    }

    #[test]
    fn mtime_selection_preserves_nanoseconds_and_has_a_stable_tie() {
        let mut newest = None;
        let base = UNIX_EPOCH + Duration::from_secs(100);
        consider_newest(&mut newest, Path::new("old.modinfo"), base);
        consider_newest(
            &mut newest,
            Path::new("new.dds"),
            base + Duration::from_nanos(1),
        );
        consider_newest(&mut newest, Path::new("older.xml"), base);
        assert_eq!(
            newest,
            Some((PathBuf::from("new.dds"), base + Duration::from_nanos(1)))
        );
        consider_newest(
            &mut newest,
            Path::new("a.dds"),
            base + Duration::from_nanos(1),
        );
        assert_eq!(
            newest,
            Some((PathBuf::from("a.dds"), base + Duration::from_nanos(1)))
        );
    }

    #[test]
    fn only_loose_dds_and_bytes_from_the_current_mod_satisfy_an_atlas() {
        let mut names = BTreeSet::from([
            "JAC_Old36".into(),
            "JAC_Crests36".into(),
            "JAC_Loose36".into(),
        ]);
        remove_loose_dds(
            &mut names,
            &[
                PathBuf::from("Textures/JAC_Loose36.DDS"),
                PathBuf::from("Art/JAC_Crests36.png"),
            ],
        );
        remove_packaged_names(Cursor::new(b"header JAC_Old36 pixels"), &mut names).unwrap();
        assert_eq!(names, BTreeSet::from(["JAC_Crests36".into()]));
        remove_packaged_names(Cursor::new(b""), &mut names).unwrap();
        assert_eq!(names, BTreeSet::from(["JAC_Crests36".into()]));
        remove_packaged_names(Cursor::new(b"JAC_Crests36"), &mut names).unwrap();
        assert!(names.is_empty());
        let mut another_mod = BTreeSet::from(["JAC_Crests36".into()]);
        remove_packaged_names(Cursor::new(b"unrelated package"), &mut another_mod).unwrap();
        assert_eq!(another_mod, BTreeSet::from(["JAC_Crests36".into()]));
    }

    #[test]
    fn blp_name_split_across_read_boundaries_is_found() {
        let mut package = vec![b'x'; 65_530];
        package.extend_from_slice(b"JAC_Crests36\0pixels");
        let mut names = BTreeSet::from(["JAC_Crests36".into(), "Missing36".into()]);
        remove_packaged_names(Cursor::new(package), &mut names).unwrap();
        assert_eq!(names, BTreeSet::from(["Missing36".into()]));
    }

    #[test]
    fn unreadable_blp_is_an_error_not_a_successful_texture_check() {
        struct Failing;
        impl Read for Failing {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "fixture denied",
                ))
            }
        }
        let mut names = BTreeSet::from(["Missing36".into()]);
        assert_eq!(
            remove_packaged_names(Failing, &mut names)
                .unwrap_err()
                .kind(),
            io::ErrorKind::PermissionDenied
        );
        assert_eq!(names, BTreeSet::from(["Missing36".into()]));
    }
}
