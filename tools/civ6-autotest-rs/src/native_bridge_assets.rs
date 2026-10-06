//! Explicit scene selection. Runtime uses the existing read-handle/hash pins.
use crate::{Error, Result};
use serde_json::{Value, json};
use std::path::PathBuf;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selection {
    pub directory: PathBuf,
    pub controller_sha256: String,
    pub dll_sha256: String,
}
impl Selection {
    pub fn from_scene(scene: &Value) -> Result<Option<Self>> {
        scene
            .get("native_bridge_assets")
            .map(Self::parse)
            .transpose()
    }
    pub fn parse(value: &Value) -> Result<Self> {
        let object = value
            .as_object()
            .ok_or_else(|| Error::scenario("native_bridge_assets must be an object"))?;
        if object.len() != 3
            || object
                .keys()
                .any(|k| !["directory", "controller_sha256", "dll_sha256"].contains(&k.as_str()))
        {
            return Err(Error::scenario(
                "native_bridge_assets requires directory and both hashes only",
            ));
        }
        let directory = value["directory"]
            .as_str()
            .filter(|s| !s.contains('\0'))
            .ok_or_else(|| Error::scenario("native directory missing"))?;
        let bytes = directory
            .strip_prefix("\\\\?\\")
            .unwrap_or(directory)
            .as_bytes();
        let windows_absolute = bytes.len() > 3
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && matches!(bytes[2], b'/' | b'\\');
        if (!windows_absolute && !std::path::Path::new(directory).is_absolute())
            || directory.split(['/', '\\']).any(|s| s == "..")
        {
            return Err(Error::scenario(
                "native directory must be absolute without parent traversal",
            ));
        }
        let hash = |key: &str| {
            value[key]
                .as_str()
                .filter(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
                .map(str::to_ascii_lowercase)
                .ok_or_else(|| Error::scenario("native frozen SHA-256 missing"))
        };
        Ok(Self {
            directory: directory.into(),
            controller_sha256: hash("controller_sha256")?,
            dll_sha256: hash("dll_sha256")?,
        })
    }
    pub fn value(&self) -> Value {
        json!({"directory":self.directory,"controller_sha256":self.controller_sha256,"dll_sha256":self.dll_sha256})
    }
    pub fn verify(&self, controller: &str, dll: &str) -> Result<()> {
        crate::native_bridge::verify_override_hash(Some(&self.controller_sha256), controller)?;
        crate::native_bridge::verify_override_hash(Some(&self.dll_sha256), dll)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn value() -> Value {
        json!({"directory":"D:/CC/Work/reader/native","controller_sha256":"a".repeat(64),"dll_sha256":"b".repeat(64)})
    }
    #[test]
    fn scene_assets_require_both_hashes_and_use_existing_verifier() {
        let valid = value();
        let selection = Selection::parse(&valid).unwrap();
        selection.verify(&"a".repeat(64), &"b".repeat(64)).unwrap();
        assert!(selection.verify(&"a".repeat(64), &"c".repeat(64)).is_err());
        assert!(selection.verify(&"c".repeat(64), &"b".repeat(64)).is_err());
        for key in ["directory", "controller_sha256", "dll_sha256"] {
            let mut bad = valid.clone();
            bad.as_object_mut().unwrap().remove(key);
            assert!(Selection::parse(&bad).is_err());
        }
        for path in ["relative", "D:/CC/../shared"] {
            let mut bad = valid.clone();
            bad["directory"] = path.into();
            assert!(Selection::parse(&bad).is_err());
        }
        let mut extended = valid;
        extended["directory"] = r"\\?\D:\CC\native".into();
        assert!(Selection::parse(&extended).is_ok());
    }
    #[test]
    fn live_scene_loads_exact_sp05_before_observation_and_preserves_eight_steps() {
        let selection = Selection::parse(&value()).unwrap();
        let scene = crate::ai_reader_case::load_scene(&selection).unwrap();
        crate::scenario::validate_scenario(&scene).unwrap();
        let steps = scene["steps"].as_array().unwrap();
        assert_eq!(steps.len(), 13);
        assert_eq!(steps[0]["action"], "restart_game");
        assert_eq!(steps[1]["action"], "enable_mods");
        assert_eq!(steps[3]["action"], "load");
        assert_eq!(steps[3]["name"], crate::ai_reader_case::SP05_SAVE);
        assert_eq!(steps[3]["save_sha256"], crate::ai_reader_case::SP05_SHA);
        assert_eq!(steps[3]["exact_name"], true);
        assert_eq!(
            steps[5..],
            crate::ai_reader_case::scenario()["steps"]
                .as_array()
                .unwrap()[..]
        );
        assert_eq!(
            steps.iter().filter(|s| s["action"] == "end_turn").count(),
            1
        );
        assert!(
            !steps
                .iter()
                .any(|s| s["action"] == "start_game" || s["action"] == "save")
        );
        assert_eq!(Selection::from_scene(&scene).unwrap(), Some(selection));
    }
    #[test]
    fn wrong_sp05_bytes_cannot_stage_assets_or_generate_a_load() {
        let dir = tempfile::tempdir().unwrap();
        let save = dir
            .path()
            .join(format!("{}.Civ6Save", crate::ai_reader_case::SP05_SAVE));
        std::fs::write(&save, b"not the real saved game").unwrap();
        let error =
            crate::ai_reader_case::prepare_load_scene(&save, dir.path(), dir.path()).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("exact independently verified SP05")
        );
        assert!(!dir.path().join("native").exists());
    }
}
