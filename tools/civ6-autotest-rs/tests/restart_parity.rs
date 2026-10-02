#[cfg(not(windows))]
use civ6_autotest_rs::ErrorKind;
use civ6_autotest_rs::{
    restart::{self, LifecycleBackend, ProcessRow},
    Error, Result,
};
use serde_json::json;
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    fs,
    path::PathBuf,
    time::{Duration, UNIX_EPOCH},
};

const MOD_ID: &str = "ba3a1c58-9287-4654-a22f-85f9e6841e9b";
const REPORTER:&str="D:\\SteamLibrary\\steamapps\\common\\Sid Meier's Civilization VI\\Base\\Binaries\\Win64Steam\\FiraxisBugReporter.exe";

#[derive(Default)]
struct MockLifecycle {
    civ: BTreeSet<u32>,
    civ_script: VecDeque<BTreeSet<u32>>,
    civ_queries: usize,
    starts: BTreeMap<u32, f64>,
    start_error: bool,
    terminated_civ: Vec<(u32, f64)>,
    reporters: BTreeSet<u32>,
    reporter_script: VecDeque<BTreeSet<u32>>,
    reporter_path: Option<String>,
    reporter_error: bool,
    reporter_terminated: Vec<u32>,
    reporter_terminate_error: Option<String>,
    uri: Vec<String>,
    window: Option<isize>,
    window_name: String,
    window_error: Option<String>,
    events: Vec<String>,
    clock: f64,
    process_rows: Vec<ProcessRow>,
    reporter_open_fails: bool,
}
impl LifecycleBackend for MockLifecycle {
    fn civ_ids(&mut self) -> Result<BTreeSet<u32>> {
        self.civ_queries += 1;
        if let Some(next) = self.civ_script.pop_front() {
            self.civ = next;
        }
        Ok(self.civ.clone())
    }
    fn process_start(&mut self, pid: u32) -> Result<f64> {
        if self.start_error {
            return Err(Error::input(format!("Cannot inspect Civ VI process {pid}")));
        }
        self.starts
            .get(&pid)
            .copied()
            .ok_or_else(|| Error::input("missing start"))
    }
    fn terminate_civ(&mut self, pid: u32, start: f64) -> Result<()> {
        self.terminated_civ.push((pid, start));
        Ok(())
    }
    fn reporter_ids(&mut self) -> Result<BTreeSet<u32>> {
        if let Some(next) = self.reporter_script.pop_front() {
            self.reporters = next;
        }
        Ok(self.reporters.clone())
    }
    fn reporter_path(&mut self, _: u32) -> Result<Option<String>> {
        if self.reporter_error {
            return Err(Error::input("Cannot inspect crash reporter 7"));
        }
        Ok(self.reporter_path.clone())
    }
    fn terminate_reporter(&mut self, pid: u32) -> Result<()> {
        if let Some(message) = &self.reporter_terminate_error {
            return Err(Error::input(message.clone()));
        }
        self.reporter_terminated.push(pid);
        self.events.push("reporter".into());
        Ok(())
    }
    fn process_rows(&mut self) -> Result<Vec<ProcessRow>> {
        self.events.push("rows".into());
        Ok(self.process_rows.clone())
    }
    fn open_reporter(&mut self, pid: u32) -> Result<Option<isize>> {
        self.events.push(format!("open:{pid}"));
        Ok((!self.reporter_open_fails).then_some(200))
    }
    fn reporter_image(&mut self, handle: isize, pid: u32) -> Result<String> {
        self.events.push(format!("image:{handle}:{pid}"));
        if self.reporter_error {
            return Err(Error::input(format!(
                "Cannot verify Civ VI helper process {pid}"
            )));
        }
        Ok(self.reporter_path.clone().unwrap())
    }
    fn terminate_reporter_handle(&mut self, handle: isize, pid: u32) -> Result<()> {
        self.events.push(format!("terminate:{handle}:{pid}"));
        if let Some(message) = &self.reporter_terminate_error {
            return Err(Error::input(message.clone()));
        }
        self.reporter_terminated.push(pid);
        Ok(())
    }
    fn close_reporter_handle(&mut self, handle: isize) {
        self.events.push(format!("close:{handle}"));
    }
    fn launch_uri(&mut self, uri: &str) -> Result<()> {
        self.uri.push(uri.into());
        self.events.push("launch".into());
        Ok(())
    }
    fn find_window(&mut self) -> Result<isize> {
        self.window.ok_or_else(|| Error::input("no window"))
    }
    fn process_name(&mut self, _: isize) -> Result<String> {
        if let Some(message) = &self.window_error {
            return Err(Error::input(message.clone()));
        }
        Ok(self.window_name.clone())
    }
    fn now(&mut self) -> f64 {
        self.clock
    }
    fn sleep(&mut self, seconds: f64) {
        self.clock += seconds;
    }
}

fn orphan_reporter(image: &str) -> MockLifecycle {
    MockLifecycle {
        process_rows: vec![ProcessRow {
            pid: 42,
            name: "firaxisbugreporter.exe".into(),
            parent_pid: 41,
        }],
        reporter_path: Some(image.into()),
        ..Default::default()
    }
}

#[test]
fn test_only_verified_orphan_helper_is_closed() {
    for image in [
        REPORTER.to_string(),
        "C:/Steam/steamapps/common/Sid Meier's Civilization VI/Base/Binaries/Win64Steam/FiraxisBugReporter.exe".into(),
        REPORTER.to_uppercase(),
    ] {
        let mut b = orphan_reporter(&image);
        assert_eq!(restart::close_orphaned_civ_reporters_with(&mut b).unwrap(), [42]);
        assert_eq!(b.reporter_terminated, [42]);
        assert_eq!(b.events, ["rows", "open:42", "image:200:42", "terminate:200:42", "close:200"]);
    }
    let mut b = orphan_reporter(REPORTER);
    b.process_rows.push(ProcessRow {
        pid: 17,
        name: "firaxisbugreporter.exe".into(),
        parent_pid: 16,
    });
    assert_eq!(
        restart::close_orphaned_civ_reporters_with(&mut b).unwrap(),
        [42, 17]
    );
    assert_eq!(b.reporter_terminated, [42, 17]);
}

#[test]
fn test_live_game_parent_preserves_reporter() {
    for parent_name in ["civilizationvi_dx12.exe", "other-parent.exe"] {
        let mut b = orphan_reporter("unused");
        b.process_rows.push(ProcessRow {
            pid: 41,
            name: parent_name.into(),
            parent_pid: 1,
        });
        assert_eq!(
            restart::close_orphaned_civ_reporters_with(&mut b).unwrap(),
            Vec::<u32>::new()
        );
        assert_eq!(b.events, ["rows"]);
        assert!(b.reporter_terminated.is_empty());
    }
}

#[test]
fn test_same_named_other_program_is_preserved() {
    for image in [
        "C:/OtherApp/FiraxisBugReporter.exe",
        "C:/Not Sid Meier's Civilization VI/Base/Binaries/Win64Steam/FiraxisBugReporter.exe",
        "C:/Steam/Sid Meier's Civilization VI/Base/Binaries/Win64Steam/Other/FiraxisBugReporter.exe",
    ] {
        let mut b = orphan_reporter(image);
        assert_eq!(restart::close_orphaned_civ_reporters_with(&mut b).unwrap(), Vec::<u32>::new());
        assert!(b.reporter_terminated.is_empty());
        assert_eq!(b.events, ["rows", "open:42", "image:200:42", "close:200"]);
    }
}

#[test]
fn test_game_appearing_during_inspection_preserves_reporter() {
    let mut b = orphan_reporter(REPORTER);
    b.process_rows.push(ProcessRow {
        pid: 43,
        name: "firaxisbugreporter.exe".into(),
        parent_pid: 1,
    });
    b.civ_script = VecDeque::from([BTreeSet::new(), BTreeSet::from([41])]);
    assert_eq!(
        restart::close_orphaned_civ_reporters_with(&mut b).unwrap(),
        Vec::<u32>::new()
    );
    assert!(b.reporter_terminated.is_empty());
    assert_eq!(b.events, ["rows", "open:42", "image:200:42", "close:200"]);
    let mut b = orphan_reporter(REPORTER);
    b.process_rows.push(ProcessRow {
        pid: 43,
        name: "firaxisbugreporter.exe".into(),
        parent_pid: 1,
    });
    b.civ_script = VecDeque::from([BTreeSet::new(), BTreeSet::new(), BTreeSet::from([41])]);
    assert_eq!(
        restart::close_orphaned_civ_reporters_with(&mut b).unwrap(),
        [42]
    );
    assert_eq!(b.reporter_terminated, [42]);
    assert_eq!(b.events.last().unwrap(), "close:200");
}

#[test]
fn test_running_game_prevents_all_cleanup() {
    let mut b = orphan_reporter(REPORTER);
    b.civ = BTreeSet::from([41]);
    assert_eq!(
        restart::close_orphaned_civ_reporters_with(&mut b).unwrap(),
        Vec::<u32>::new()
    );
    assert!(b.events.is_empty());
    assert!(b.reporter_terminated.is_empty());
}

#[test]
fn pinned_reporter_handle_is_released_on_errors_and_failed_open_is_skipped() {
    let mut b = orphan_reporter(REPORTER);
    b.reporter_open_fails = true;
    assert_eq!(
        restart::close_orphaned_civ_reporters_with(&mut b).unwrap(),
        Vec::<u32>::new()
    );
    assert_eq!(b.events, ["rows", "open:42"]);
    for image_failure in [true, false] {
        let mut b = orphan_reporter(REPORTER);
        b.reporter_error = image_failure;
        b.reporter_terminate_error = Some("Could not close orphan Civ VI helper 42".into());
        let error = restart::close_orphaned_civ_reporters_with(&mut b).unwrap_err();
        assert!(error.message.contains(if image_failure {
            "Cannot verify"
        } else {
            "Could not close"
        }));
        assert_eq!(b.events.last().unwrap(), "close:200");
        assert_eq!(
            b.events
                .iter()
                .filter(|event| *event == "close:200")
                .count(),
            1
        );
        assert!(b.reporter_terminated.is_empty());
    }
}

#[test]
fn only_civ6_reporter_is_closed_without_running_game() {
    for (path, appears, closed) in [
        (REPORTER, false, true),
        (REPORTER.to_uppercase().as_str(), false, true),
        ("D:\\OtherGame\\FiraxisBugReporter.exe", false, false),
        (REPORTER, true, false),
    ] {
        let mut b = MockLifecycle {
            reporters: BTreeSet::from([7]),
            reporter_path: Some(path.into()),
            civ_script: VecDeque::from([
                BTreeSet::new(),
                if appears {
                    BTreeSet::from([42])
                } else {
                    BTreeSet::new()
                },
            ]),
            ..Default::default()
        };
        restart::close_orphan_civ_reporters_with(&mut b).unwrap();
        assert_eq!(b.reporter_terminated, if closed { vec![7] } else { vec![] });
    }
}

#[test]
fn running_game_keeps_reporters() {
    let mut b = MockLifecycle {
        civ: BTreeSet::from([42]),
        reporters: BTreeSet::from([7]),
        reporter_path: Some(REPORTER.into()),
        ..Default::default()
    };
    restart::close_orphan_civ_reporters_with(&mut b).unwrap();
    assert!(b.reporter_terminated.is_empty());
}

#[test]
fn unreadable_reporter_is_not_terminated() {
    let mut b = MockLifecycle {
        reporters: BTreeSet::from([7]),
        reporter_error: true,
        ..Default::default()
    };
    assert!(restart::close_orphan_civ_reporters_with(&mut b)
        .unwrap_err()
        .message
        .contains("Cannot inspect crash reporter 7"));
    assert!(b.reporter_terminated.is_empty());
}

#[test]
fn exited_reporter_is_ignored() {
    let mut b = MockLifecycle {
        reporter_script: VecDeque::from([BTreeSet::from([7]), BTreeSet::new()]),
        ..Default::default()
    };
    restart::close_orphan_civ_reporters_with(&mut b).unwrap();
    assert!(b.reporter_terminated.is_empty());
}

#[test]
fn remaining_reporter_that_cannot_open_stops_launch() {
    let mut b = MockLifecycle {
        reporters: BTreeSet::from([7]),
        ..Default::default()
    };
    assert!(restart::launch_via_steam_with(&mut b)
        .unwrap_err()
        .message
        .contains("Cannot open remaining crash reporter 7"));
    assert!(b.uri.is_empty() && b.reporter_terminated.is_empty());
}

#[test]
fn reporter_must_finish_exiting_before_launch() {
    assert!(restart::reporter_finished(0));
    assert!(!restart::reporter_finished(0x102));
    assert!(!restart::reporter_finished(0xffff_ffff));
    for detail in ["timeout", "wait failed"] {
        let mut b = MockLifecycle {
            reporters: BTreeSet::from([7]),
            reporter_path: Some(REPORTER.into()),
            reporter_terminate_error: Some(format!(
                "Orphan Civ VI crash reporter 7 did not finish exiting: {detail}"
            )),
            ..Default::default()
        };
        assert!(restart::launch_via_steam_with(&mut b)
            .unwrap_err()
            .message
            .contains("did not finish exiting"));
        assert!(b.uri.is_empty());
    }
}

#[test]
fn steam_launch_clears_reporters_before_sending_dx11_uri() {
    let mut b = MockLifecycle {
        reporters: BTreeSet::from([7]),
        reporter_path: Some(REPORTER.into()),
        ..Default::default()
    };
    restart::launch_via_steam_with(&mut b).unwrap();
    assert_eq!(b.events, ["reporter", "launch"]);
    assert_eq!(b.uri, ["steam://launch/289070/option2"]);
}

#[test]
fn close_failure_stops_steam_launch() {
    let mut b = MockLifecycle {
        reporters: BTreeSet::from([7]),
        reporter_path: Some(REPORTER.into()),
        reporter_terminate_error: Some("Could not close orphan Civ VI crash reporter 7".into()),
        ..Default::default()
    };
    assert!(restart::launch_via_steam_with(&mut b).is_err());
    assert!(b.uri.is_empty());
}

#[test]
fn only_dx11_window_is_accepted_after_launch() {
    for (name, accepted) in [
        ("civilizationvi.exe", true),
        ("civilizationvi_dx12.exe", false),
        ("other.exe", false),
    ] {
        let mut b = MockLifecycle {
            window_name: name.into(),
            ..Default::default()
        };
        let result = restart::require_dx11_window_with(&mut b, 123);
        assert_eq!(result.is_ok(), accepted);
        if !accepted {
            assert!(result
                .unwrap_err()
                .message
                .contains("expected CivilizationVI.exe"));
        }
    }
}

#[test]
fn unreadable_window_is_not_accepted() {
    let mut b = MockLifecycle {
        window_error: Some("Cannot read executable".into()),
        ..Default::default()
    };
    assert!(restart::require_dx11_window_with(&mut b, 123)
        .unwrap_err()
        .message
        .contains("Cannot read executable"));
}

#[test]
fn freshness_decision_uses_strictly_newer_mod_file() {
    let newest = (PathBuf::from("changed.dds"), 200.0);
    assert_eq!(
        restart::restart_reason(None, None).as_deref(),
        Some("Civ6 is not running")
    );
    assert!(restart::restart_reason(Some(&newest), Some(199.0))
        .unwrap()
        .contains("changed.dds"));
    for start in [200.0, 201.0] {
        assert_eq!(restart::restart_reason(Some(&newest), Some(start)), None);
    }
    assert_eq!(restart::restart_reason(None, Some(100.0)), None);
}

#[test]
fn mod_scan_resolves_folder_and_finds_newest_file() {
    let temp = tempfile::tempdir().unwrap();
    let mods = temp.path().join("Mods");
    let folder = mods.join("MyMod");
    fs::create_dir_all(folder.join("Art")).unwrap();
    let info = folder.join("MyMod.modinfo");
    fs::write(&info, format!("<Mod id=\"{MOD_ID}\" version=\"1\"/>")).unwrap();
    let newer = folder.join("Art").join("icon.dds");
    fs::write(&newer, b"test").unwrap();
    fs::File::options()
        .write(true)
        .open(&info)
        .unwrap()
        .set_modified(UNIX_EPOCH + Duration::from_secs(100))
        .unwrap();
    fs::File::options()
        .write(true)
        .open(&newer)
        .unwrap()
        .set_modified(UNIX_EPOCH + Duration::from_secs(200))
        .unwrap();
    let scenario = json!({"steps":[{"action":"enable_mods","ids":[MOD_ID.to_uppercase()]}]});
    let folders = restart::enabled_mod_folders(&scenario, &mods).unwrap();
    assert_eq!(folders, vec![folder.canonicalize().unwrap()]);
    assert_eq!(
        restart::newest_mod_file(&folders).unwrap(),
        Some((newer.canonicalize().unwrap(), 200.0))
    );
    assert!(restart::restart_reason(
        restart::newest_mod_file(&folders).unwrap().as_ref(),
        Some(150.0)
    )
    .unwrap()
    .contains("icon.dds"));
}

#[test]
fn missing_or_duplicate_enabled_mod_is_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let mods = temp.path();
    let scenario = json!({"steps":[{"action":"enable_mods","ids":[MOD_ID]}]});
    assert!(restart::enabled_mod_folders(&scenario, mods)
        .unwrap_err()
        .message
        .contains("not installed"));
    for name in ["a", "b"] {
        let dir = mods.join(name);
        fs::create_dir(&dir).unwrap();
        fs::write(dir.join("mod.modinfo"), format!("<Mod id=\"{MOD_ID}\"/>")).unwrap();
    }
    assert!(restart::enabled_mod_folders(&scenario, mods)
        .unwrap_err()
        .message
        .contains("Duplicate"));
}

#[test]
fn atlas_images_missing_from_mod_packages_are_reported() {
    let temp = tempfile::tempdir().unwrap();
    let folder = temp.path().join("MyMod");
    fs::create_dir_all(folder.join("Data")).unwrap();
    fs::write(folder.join("Data/Icons.xml"),"<GameInfo><IconTextureAtlases><Row Filename=\"JAC_Old36.dds\"/><Row Filename=\"JAC_Crests36.dds\"/><Row Filename=\"JAC_Loose36.dds\"/></IconTextureAtlases></GameInfo>").unwrap();
    fs::create_dir_all(folder.join("Textures")).unwrap();
    fs::write(folder.join("Textures/JAC_Loose36.dds"), b"dds").unwrap();
    let blp = folder.join("Icons.blp");
    fs::write(&blp, b"header JAC_Old36 pixels").unwrap();
    assert_eq!(
        restart::missing_atlas_textures(&[folder.clone()]).unwrap(),
        ["MyMod: JAC_Crests36"]
    );
    fs::write(&blp, b"JAC_Old36 JAC_Crests36").unwrap();
    assert!(restart::missing_atlas_textures(&[folder])
        .unwrap()
        .is_empty());
}

#[test]
fn single_process_waits_for_exiting_process_to_disappear() {
    let mut b = MockLifecycle {
        civ_script: VecDeque::from([
            BTreeSet::from([1, 2]),
            BTreeSet::from([1, 2]),
            BTreeSet::from([2]),
        ]),
        starts: BTreeMap::from([(2, 100.0)]),
        ..Default::default()
    };
    assert_eq!(
        restart::single_civ_process_with(&mut b, 30.0).unwrap(),
        Some((2, 100.0))
    );
    assert!(b.clock > 0.0);
}

#[test]
fn single_process_refuses_persistent_duplicates() {
    let mut b = MockLifecycle {
        civ: BTreeSet::from([1, 2]),
        ..Default::default()
    };
    assert!(restart::single_civ_process_with(&mut b, 0.0)
        .unwrap_err()
        .message
        .contains("Multiple Civ VI processes"));
}

#[test]
fn window_wait_ignores_transient_duplicate_processes() {
    let mut b = MockLifecycle {
        civ_script: VecDeque::from([BTreeSet::from([1, 2]), BTreeSet::from([12])]),
        starts: BTreeMap::from([(12, 100.0)]),
        window: Some(123),
        ..Default::default()
    };
    assert_eq!(
        restart::wait_for_civ_window_with(&mut b, 10.0).unwrap(),
        123
    );
}

#[test]
fn force_termination_requires_same_pid_start_and_unique_process() {
    let mut b = MockLifecycle {
        civ: BTreeSet::from([12]),
        starts: BTreeMap::from([(12, 100.0)]),
        ..Default::default()
    };
    assert!(restart::terminate_civ_process_with(&mut b, 12, 99.0)
        .unwrap_err()
        .message
        .contains("changed"));
    assert!(b.terminated_civ.is_empty());
    restart::terminate_civ_process_with(&mut b, 12, 100.0).unwrap();
    assert_eq!(b.terminated_civ, [(12, 100.0)]);
    b.civ.insert(13);
    assert!(restart::terminate_civ_process_with(&mut b, 12, 100.0)
        .unwrap_err()
        .message
        .contains("refusing termination"));
    assert_eq!(b.terminated_civ.len(), 1);
}

#[test]
fn exited_process_does_not_trigger_termination() {
    let mut b = MockLifecycle::default();
    restart::terminate_civ_process_with(&mut b, 12, 100.0).unwrap();
    assert!(b.terminated_civ.is_empty());
}

#[test]
fn wait_for_exit_observes_disappearance_and_deadline() {
    let mut b = MockLifecycle {
        civ_script: VecDeque::from([BTreeSet::from([12]), BTreeSet::new()]),
        ..Default::default()
    };
    assert!(restart::wait_for_exit_with(&mut b, 12, 2.0).unwrap());
    let mut b = MockLifecycle {
        civ: BTreeSet::from([12]),
        ..Default::default()
    };
    assert!(!restart::wait_for_exit_with(&mut b, 12, 1.0).unwrap());
}

#[cfg(not(windows))]
#[test]
fn non_windows_lifecycle_reports_platform_error() {
    assert_eq!(
        restart::single_civ_process(0.0).unwrap_err().kind,
        ErrorKind::Input
    );
}
