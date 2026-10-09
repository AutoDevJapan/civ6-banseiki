use civ6_autotest_rs::{
    click::{self, InputBackend, KeyDelivery},
    ErrorKind, Result,
};
use std::collections::BTreeSet;

struct MockInput {
    ids: BTreeSet<u32>,
    windows: Vec<(isize, String)>,
    executable: String,
    foreground: isize,
    foreground_calls: usize,
    lose_focus_at: Option<usize>,
    expire_at_foreground: Option<usize>,
    activated: bool,
    clock: f64,
    expire_on_scan: bool,
    expire_on_sleep: bool,
    lose_focus_on_sleep: bool,
    changed_window_after: Option<usize>,
    windows_calls: usize,
    post: Vec<(isize, u32, usize, isize)>,
    send: Vec<bool>,
    mouse: Vec<u32>,
    cursor: Vec<(i32, i32)>,
    scans: usize,
}
impl Default for MockInput {
    fn default() -> Self {
        Self {
            ids: BTreeSet::from([12]),
            windows: vec![(100, click::TITLE_PREFIX.to_string())],
            executable: "civilizationvi.exe".into(),
            foreground: 100,
            foreground_calls: 0,
            lose_focus_at: None,
            expire_at_foreground: None,
            activated: true,
            clock: 0.0,
            expire_on_scan: false,
            expire_on_sleep: false,
            lose_focus_on_sleep: false,
            changed_window_after: None,
            windows_calls: 0,
            post: Vec::new(),
            send: Vec::new(),
            mouse: Vec::new(),
            cursor: Vec::new(),
            scans: 0,
        }
    }
}
impl InputBackend for MockInput {
    fn process_ids(&mut self, names: &BTreeSet<String>) -> Result<BTreeSet<u32>> {
        assert_eq!(*names, click::civ_names());
        Ok(self.ids.clone())
    }
    fn windows(&mut self) -> Result<Vec<(isize, String)>> {
        self.windows_calls += 1;
        if self
            .changed_window_after
            .is_some_and(|n| self.windows_calls >= n)
        {
            return Ok(vec![(200, click::TITLE_PREFIX.into())]);
        }
        Ok(self.windows.clone())
    }
    fn process_name(&mut self, _: isize) -> Result<String> {
        Ok(self.executable.clone())
    }
    fn foreground(&mut self) -> isize {
        self.foreground_calls += 1;
        if self.expire_at_foreground == Some(self.foreground_calls) {
            self.clock = 1.0;
        }
        if self.lose_focus_at == Some(self.foreground_calls) {
            self.foreground = 999;
        }
        self.foreground
    }
    fn restore(&mut self, _: isize) {}
    fn activate(&mut self, _: isize) -> bool {
        self.activated
    }
    fn rect(&mut self, _: isize) -> Result<(i32, i32, i32, i32)> {
        Ok((10, 20, 210, 120))
    }
    fn cursor(&mut self, x: i32, y: i32) -> bool {
        self.cursor.push((x, y));
        true
    }
    fn mouse(&mut self, flag: u32) {
        self.mouse.push(flag);
    }
    fn scan(&mut self, _: u16) -> u32 {
        self.scans += 1;
        if self.expire_on_scan {
            self.clock = 1.0;
        }
        1
    }
    fn post(&mut self, h: isize, m: u32, vk: usize, p: isize) -> bool {
        self.post.push((h, m, vk, p));
        true
    }
    fn send(&mut self, _: u16, up: bool) -> bool {
        self.send.push(up);
        true
    }
    fn now(&mut self) -> f64 {
        self.clock
    }
    fn sleep(&mut self, seconds: f64) {
        if self.expire_on_sleep {
            self.clock += seconds;
        }
        if self.lose_focus_on_sleep {
            self.foreground = 999;
        }
    }
}

#[test]
fn deadline_expiring_during_focus_sends_no_key() {
    let mut b = MockInput {
        expire_on_sleep: true,
        ..Default::default()
    };
    let e = click::press_key_with(&mut b, click::VK_ESCAPE, Some(0.3), KeyDelivery::SendInput)
        .unwrap_err();
    assert_eq!(e.kind, ErrorKind::Input);
    assert!(e.message.contains("deadline"));
    assert!(b.post.is_empty() && b.send.is_empty());
}

#[test]
fn deadline_expiring_after_key_mapping_sends_no_window_key() {
    let mut b = MockInput {
        expire_on_scan: true,
        ..Default::default()
    };
    let e = click::press_key_with(&mut b, click::VK_ESCAPE, Some(1.0), KeyDelivery::Window)
        .unwrap_err();
    assert!(e.message.contains("deadline"));
    assert_eq!(b.scans, 1);
    assert!(b.post.is_empty());
}

#[test]
fn deadline_expiring_after_foreground_check_sends_no_sendinput_key() {
    let mut b = MockInput {
        expire_at_foreground: Some(2),
        ..Default::default()
    };
    let e = click::press_key_with(&mut b, click::VK_ESCAPE, Some(1.0), KeyDelivery::SendInput)
        .unwrap_err();
    assert!(e.message.contains("deadline"));
    assert_eq!(b.foreground_calls, 2);
    assert!(b.send.is_empty());
}

#[test]
fn key_before_deadline_is_sent_in_both_modes() {
    for mode in [KeyDelivery::Window, KeyDelivery::SendInput] {
        let mut b = MockInput::default();
        click::press_key_with(&mut b, click::VK_ESCAPE, Some(1.0), mode).unwrap();
        match mode {
            KeyDelivery::Window => {
                assert_eq!(
                    b.post.iter().map(|p| p.1).collect::<Vec<_>>(),
                    vec![click::WM_KEYDOWN, click::WM_KEYUP]
                );
                assert!(b.send.is_empty());
            }
            KeyDelivery::SendInput => {
                assert_eq!(b.send, vec![false, true]);
                assert!(b.post.is_empty());
            }
        }
    }
}

#[test]
fn foreground_mismatch_sends_no_keys_or_clicks() {
    let mut b = MockInput {
        foreground: 999,
        ..Default::default()
    };
    assert!(
        click::press_key_with(&mut b, click::VK_ESCAPE, None, KeyDelivery::SendInput)
            .unwrap_err()
            .message
            .contains("foreground")
    );
    assert!(click::click_fraction_with(&mut b, 0.5, 0.5)
        .unwrap_err()
        .message
        .contains("foreground"));
    assert!(b.send.is_empty() && b.post.is_empty() && b.mouse.is_empty() && b.cursor.is_empty());
}

#[test]
fn window_key_releases_after_foreground_changes() {
    let mut b = MockInput {
        lose_focus_on_sleep: true,
        ..Default::default()
    };
    click::press_key_with(&mut b, click::VK_ESCAPE, None, KeyDelivery::Window).unwrap();
    assert_eq!(
        b.post,
        vec![
            (
                100,
                click::WM_KEYDOWN,
                click::VK_ESCAPE as usize,
                1 | (1 << 16)
            ),
            (
                100,
                click::WM_KEYUP,
                click::VK_ESCAPE as usize,
                1 | (1 << 16) | 0xC000_0000
            )
        ]
    );
    assert_eq!(b.foreground, 999);
}

#[test]
fn window_key_targets_verified_game_while_background() {
    let mut b = MockInput {
        foreground: 999,
        ..Default::default()
    };
    click::press_key_with(&mut b, click::VK_ESCAPE, None, KeyDelivery::Window).unwrap();
    assert_eq!(b.post.len(), 2);
    assert!(b.send.is_empty());
    assert_eq!(b.foreground_calls, 0);
}

#[test]
fn window_key_rechecks_target_before_background_input() {
    let mut b = MockInput {
        changed_window_after: Some(2),
        ..Default::default()
    };
    assert!(
        click::press_key_with(&mut b, click::VK_ESCAPE, None, KeyDelivery::Window)
            .unwrap_err()
            .message
            .contains("window changed")
    );
    assert!(b.post.is_empty());
}

#[test]
fn sendinput_fallback_releases_key_after_focus_loss() {
    let mut b = MockInput {
        lose_focus_at: Some(3),
        ..Default::default()
    };
    assert!(
        click::press_key_with(&mut b, click::VK_ESCAPE, None, KeyDelivery::SendInput)
            .unwrap_err()
            .message
            .contains("foreground")
    );
    assert_eq!(b.send, vec![false, true]);
    assert!(b.post.is_empty());
}

#[test]
fn failed_foreground_activation_sends_no_input() {
    let mut b = MockInput {
        activated: false,
        foreground: 999,
        ..Default::default()
    };
    assert!(
        click::press_key_with(&mut b, click::VK_ESCAPE, None, KeyDelivery::SendInput)
            .unwrap_err()
            .message
            .contains("Could not bring")
    );
    assert!(b.send.is_empty() && b.post.is_empty());
}

#[test]
fn already_foreground_game_can_be_clicked_if_activation_reports_false() {
    let mut b = MockInput {
        activated: false,
        ..Default::default()
    };
    assert_eq!(
        click::click_fraction_with(&mut b, 0.5, 0.5).unwrap(),
        (110, 70)
    );
    assert_eq!(b.mouse, vec![click::LEFT_DOWN, click::LEFT_UP]);
}

#[test]
fn window_must_have_unique_civ_executable() {
    let mut b = MockInput {
        windows: vec![
            (100, click::TITLE_PREFIX.into()),
            (200, click::TITLE_PREFIX.into()),
        ],
        ..Default::default()
    };
    assert!(click::find_civ_window_with(&mut b)
        .unwrap_err()
        .message
        .contains("found 2"));
    b.executable = "civilizationvihelper.exe".into();
    assert!(click::find_civ_window_with(&mut b)
        .unwrap_err()
        .message
        .contains("found 0"));
    assert!(click::press_key_with(&mut b, click::VK_ESCAPE, None, KeyDelivery::Window).is_err());
    assert!(b.post.is_empty());
    b.windows = vec![(100, click::TITLE_PREFIX.into())];
    b.executable = "civilizationvi_dx12.exe".into();
    assert_eq!(click::find_civ_window_with(&mut b).unwrap(), 100);
}

#[test]
fn hidden_second_civ_process_blocks_input_before_window_scan() {
    let mut b = MockInput {
        ids: BTreeSet::from([12, 13]),
        ..Default::default()
    };
    assert!(
        click::press_key_with(&mut b, click::VK_ESCAPE, None, KeyDelivery::Window)
            .unwrap_err()
            .message
            .contains("Multiple Civ VI processes")
    );
    assert_eq!(b.windows_calls, 0);
    assert!(b.post.is_empty());
}

#[test]
fn process_snapshot_counts_both_civ_executables() {
    let names = click::civ_names();
    let found: [(u32, &str); 3] = [
        (11, "CivilizationVI.exe"),
        (12, "CivilizationVI_DX12.exe"),
        (13, "CivilizationVIHelper.exe"),
    ];
    let ids: Vec<_> = found
        .into_iter()
        .filter(|(_, name)| click::matches_executable(name, &names))
        .map(|(id, _)| id)
        .collect();
    assert_eq!(ids, vec![11, 12]);
}

#[test]
fn click_releases_button_after_focus_loss() {
    let mut b = MockInput {
        lose_focus_at: Some(4),
        ..Default::default()
    };
    assert!(click::click_fraction_with(&mut b, 0.5, 0.5).is_err());
    assert_eq!(b.cursor, vec![(110, 70)]);
    assert_eq!(b.mouse, vec![click::LEFT_DOWN, click::LEFT_UP]);
}

#[test]
fn invalid_fraction_never_queries_or_sends() {
    let mut b = MockInput::default();
    assert!(click::click_fraction_with(&mut b, f64::NAN, 0.5).is_err());
    assert_eq!(b.windows_calls, 0);
}

#[cfg(not(windows))]
#[test]
fn non_windows_public_api_fails_explicitly() {
    assert!(click::find_civ_window()
        .unwrap_err()
        .message
        .contains("Windows"));
}
