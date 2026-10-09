//! Finite, std-only policy for an already captured Steam dialog snapshot.
//!
//! No UI, filesystem, clock or Steam operations occur here. `Select` indexes the
//! original `buttons` slice. The caller supplies console binding evidence:
//! exactly one AppID 289070 LaunchApp waiting on SynchronizingControllerConfig
//! syncfailed. Either a fresh console suffix proves this, or the current
//! registered Steam process's final active GameAction proves it on reads before
//! and after the snapshot. The latter may be old, but a later other-game action
//! invalidates it. Process identity and this evidence are checked by the caller.
//! A console proof cannot authorize arbitrary text or another game's window.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    None,
    Wait,
    Select(usize),
    Stop(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    UnableToSync,
    Conflict,
    Syncing,
    Offline,
    Unknown,
}

// Order longest first: a short name must not leave an unvalidated prefix behind.
const CIV6_NAMES: &[&str] = &[
    "sid meier's civilization vi",
    "シドマイヤーズ シヴィライゼーション vi",
    "シドマイヤーズ シヴィライゼーションvi",
    "civilization vi",
    "シヴィライゼーション vi",
    "シヴィライゼーションvi",
];

// Transcribed from live176's Steam screenshot, not the abbreviated proof JSON.
const STOCK_FAILURE: &str = "steam was unable to sync your saves with the steam cloud. if you play now, you may not have previous game progress and you may permanently lose it.";

fn normalize(text: &str) -> String {
    let text = text
        .replace(['\u{2018}', '\u{2019}'], "'")
        .replace(['\u{00ae}', '\u{2122}'], "");
    text.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn game_title(title: &str) -> bool {
    CIV6_NAMES.iter().any(|name| {
        [" - ", " – ", " — ", ": ", " | "].iter().any(|separator| {
            title == format!("steam{separator}{name}") || title == format!("{name}{separator}steam")
        })
    })
}

fn generic_title(title: &str) -> bool {
    matches!(
        title,
        "steam" | "steam - unable to sync" | "unable to sync - steam"
    )
}

fn steam_title(title: &str) -> bool {
    title
        .split(|ch: char| !ch.is_alphanumeric())
        .any(|word| word == "steam")
}

fn without_heading<'a>(body: &'a str, headings: &[&str]) -> &'a str {
    for heading in headings {
        if let Some(rest) = body.strip_prefix(heading) {
            // A heading must end or have a delimiter, not be a word prefix.
            if rest.is_empty() || rest.starts_with([' ', ':', '：', '\n']) {
                return rest.trim_start_matches([' ', ':', '：', '\n']);
            }
        }
    }
    body
}

fn failure_body(body: &str) -> &str {
    let body = without_heading(body, &["unable to sync", "同期できません", "同期失敗"]);
    body.strip_prefix("warning - ").unwrap_or(body)
}

fn actual_complete_warning(body: &str) -> bool {
    failure_body(body) == STOCK_FAILURE
        // Native UIA probe: Steam root plus three copies of the modal heading.
        // Accept this measured prefix exactly, not arbitrary duplicated text.
        || body.strip_prefix("steam unable to sync unable to sync unable to sync warning - ")
            == Some(STOCK_FAILURE)
}

// Replace only complete allowlisted game names. Everything left over still has
// to match a complete finite template, so even an unknown other game is rejected.
fn template_body(body: &str) -> String {
    let mut result = body.to_owned();
    for name in CIV6_NAMES {
        result = result.replace(name, "{civ6}");
    }
    result
}

fn explicit_kind(body: &str) -> Kind {
    if body.contains("{civ6}") {
        return Kind::Unknown;
    }
    if actual_complete_warning(body) {
        return Kind::UnableToSync;
    }
    let body = template_body(body);
    let failure = failure_body(&body);
    if failure == STOCK_FAILURE
        || matches!(
            failure,
            "steam was unable to sync your saves with the steam cloud."
                | "steam was unable to sync your files for {civ6} with the steam cloud."
                | "steam was unable to sync your files for {civ6} with the steam cloud. if you launch this application now, you may lose game progress or settings."
                | "steamは{civ6}のファイルをsteamクラウドと同期できませんでした。"
                | "steam は {civ6} のファイルを steam クラウドと同期できませんでした。"
                | "steamはセーブデータをsteamクラウドと同期できませんでした。"
        )
    {
        return Kind::UnableToSync;
    }
    let conflict = without_heading(
        &body,
        &["steam cloud conflict", "cloud conflict", "クラウドの競合"],
    );
    if matches!(
        conflict,
        "your local files conflict with the ones stored in the steam cloud."
            | "your local {civ6} files conflict with the ones stored in the steam cloud."
            | "the steam cloud files and local files are different. choose which files you wish to keep."
            | "ローカルファイルとsteamクラウドのファイルが競合しています。"
            | "{civ6}のローカルファイルとsteamクラウドのファイルが競合しています。"
    ) {
        return Kind::Conflict;
    }
    if matches!(
        body.as_str(),
        "syncing"
            | "syncing with steam cloud..."
            | "synchronizing with steam cloud..."
            | "syncing {civ6} with steam cloud..."
            | "steamクラウドと同期中..."
            | "{civ6}をsteamクラウドと同期中..."
    ) {
        return Kind::Syncing;
    }
    if matches!(
        body.as_str(),
        "steam is in offline mode."
            | "steam is currently in offline mode."
            | "steam is in offline mode. you can play {civ6} offline."
            | "steamはオフラインモードです。"
            | "steamはオフラインモードです。{civ6}をオフラインでプレイできます。"
    ) {
        return Kind::Offline;
    }
    Kind::Unknown
}

/// Classifies recognized warning text without authorizing an action. Unknown
/// wording stays Unknown; the console fallback requires the *entire* measured
/// stock warning, even if its first sentence resembles another known template.
pub fn classify(title: &str, body: &str) -> Kind {
    let title = normalize(title);
    let body = normalize(body);
    if body.is_empty() || !steam_title(&title) {
        return Kind::Unknown;
    }
    if game_title(&title) {
        explicit_kind(&body)
    } else if generic_title(&title) && actual_complete_warning(&body) {
        Kind::UnableToSync
    } else {
        Kind::Unknown
    }
}

fn warning_like(title: &str, body: &str, buttons: &[String]) -> bool {
    [
        "unable to sync",
        "syncfailed",
        "steam cloud",
        "cloud conflict",
        "syncing",
        "synchronizing",
        "offline",
        "同期",
        "競合",
        "クラウド",
        "オフライン",
    ]
    .iter()
    .any(|marker| title.contains(marker) || body.contains(marker))
        || buttons.iter().any(|button| {
            let button = normalize(button);
            matches!(
                button.as_str(),
                "play anyway"
                    | "それでもプレイ"
                    | "upload to steam cloud"
                    | "local files"
                    | "ローカルファイル"
            )
        })
}

fn ordinary_window(title: &str, buttons: &[String]) -> bool {
    matches!(title, "steam" | "steam - library" | "steam library")
        && buttons.iter().all(|button| {
            matches!(
                normalize(button).as_str(),
                "store" | "library" | "community" | "ストア" | "ライブラリ" | "コミュニティ"
            )
        })
}

fn updating_client(title: &str, body: &str, buttons: &[String]) -> bool {
    // live94 captured Steam's registered updater, not a save conflict. Waiting
    // does not invoke Cancel/Close or choose any copy of the user's files. The
    // caller's existing launch deadline still bounds this wait.
    if title != "steam"
        || !matches!(
            body,
            "updating steam... updating steam... verifying installation... system system"
                | "updating steam... verifying installation..."
        )
    {
        return false;
    }
    let mut labels = buttons.iter().map(|b| normalize(b)).collect::<Vec<_>>();
    labels.sort();
    labels == ["cancel", "close", "minimize"]
}

fn safe_button(kind: Kind, label: &str) -> bool {
    match kind {
        Kind::UnableToSync | Kind::Offline => {
            matches!(
                label,
                "play anyway" | "それでもプレイ" | "それでもプレイする"
            )
        }
        Kind::Conflict => {
            matches!(
                label,
                "upload to steam cloud" | "local files" | "ローカルファイル"
            )
        }
        _ => false,
    }
}

fn other_button(kind: Kind, label: &str) -> bool {
    if matches!(label, "cancel" | "キャンセル") {
        return true;
    }
    match kind {
        Kind::UnableToSync => matches!(label, "retry sync" | "retry" | "try again" | "再試行"),
        Kind::Conflict => matches!(
            label,
            "download to this machine"
                | "download to this computer"
                | "download from steam cloud"
                | "cloud files"
                | "クラウドファイル"
                | "steamクラウドからダウンロード"
                | "ダウンロード"
        ),
        _ => false,
    }
}

/// Makes a bounded, deterministic choice from one snapshot. No game attribution
/// is inferred from buttons. `console_proves_civ6` is ignored unless both the
/// generic Steam title and live176's complete warning match first.
pub fn decide(title: &str, body: &str, buttons: &[String], console_proves_civ6: bool) -> Action {
    let title = normalize(title);
    let body = normalize(body);
    if !steam_title(&title) {
        return Action::None;
    }
    if updating_client(&title, &body, buttons) {
        return Action::Wait;
    }
    let explicit = game_title(&title);
    if !explicit && ordinary_window(&title, buttons) && !warning_like(&title, &body, buttons) {
        return Action::None;
    }
    if body.is_empty() {
        return Action::Stop("Steam warning body is empty".into());
    }
    // Match actual warning BEFORE consulting console evidence.
    let kind = if explicit {
        classify(&title, &body)
    } else if generic_title(&title) && actual_complete_warning(&body) {
        if !console_proves_civ6 {
            return Action::Stop("Complete Steam warning lacks Civ6 console binding proof".into());
        }
        Kind::UnableToSync
    } else {
        return Action::Stop("Steam warning is not attributable solely to Civ6".into());
    };
    match kind {
        Kind::Syncing => return Action::Wait,
        Kind::Unknown => return Action::Stop("Unknown or mixed-game Civ6 warning body".into()),
        _ => {}
    }
    let mut selected = None;
    for (index, button) in buttons.iter().enumerate() {
        let label = normalize(button);
        if safe_button(kind, &label) {
            if selected.replace(index).is_some() {
                return Action::Stop("Multiple safe Steam buttons".into());
            }
        } else if !other_button(kind, &label) {
            return Action::Stop("Unknown Steam button".into());
        }
    }
    match selected {
        Some(index) => Action::Select(index),
        None => Action::Stop("No unique safe Steam button".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TITLE: &str = "Steam - Sid Meier's Civilization VI";
    const FILES: &str =
        "Steam was unable to sync your files for Sid Meier's Civilization VI with the Steam Cloud.";
    const CONFLICT: &str = "Your local files conflict with the ones stored in the Steam Cloud.";
    // Keep the fixture independently transcribed, not generated from the policy.
    const ACTUAL: &str = "Unable to Sync\nWarning - Steam was unable to sync your saves with the Steam Cloud.\n\nIf you play now, you may not have previous game progress and you may permanently lose it.";
    const ACTUAL_UIA: &str = "Steam\nUnable to Sync\nUnable to Sync\nUnable to Sync\nWarning - Steam was unable to sync your saves with the Steam Cloud.\nIf you play now, you may not have previous game progress and you may permanently lose it.\n";

    fn buttons(labels: &[&str]) -> Vec<String> {
        labels.iter().map(|label| (*label).into()).collect()
    }
    fn stopped(action: Action) {
        assert!(matches!(action, Action::Stop(reason) if !reason.is_empty()));
    }

    #[test]
    fn actual_warning_requires_console_proof_for_generic_steam() {
        assert_eq!(classify("Steam", ACTUAL), Kind::UnableToSync);
        let b = buttons(&["Cancel", "Play anyway"]);
        stopped(decide("Steam", ACTUAL, &b, false));
        assert_eq!(decide("Steam", ACTUAL, &b, true), Action::Select(1));
    }

    #[test]
    fn measured_native_uia_warning_selects_only_with_binding_proof() {
        let b = buttons(&["Play anyway", "Cancel"]);
        assert_eq!(classify("Steam", ACTUAL_UIA), Kind::UnableToSync);
        assert_eq!(decide("Steam", ACTUAL_UIA, &b, true), Action::Select(0));
        stopped(decide("Steam", ACTUAL_UIA, &b, false));
        assert_eq!(decide(TITLE, ACTUAL_UIA, &b, false), Action::Select(0));
    }

    #[test]
    fn native_uia_boilerplate_does_not_authorize_extra_or_partial_text() {
        for body in [
            format!("{ACTUAL_UIA}Dota 2"),
            ACTUAL_UIA.replacen("Steam\n", "Steam\nDota 2\n", 1),
            ACTUAL_UIA.replace(
                "Unable to Sync\nUnable to Sync\nUnable to Sync",
                "Unable to Sync\nUnknown dialog\nUnable to Sync",
            ),
            ACTUAL_UIA.replace("and you may permanently lose it.", ""),
        ] {
            assert_eq!(classify("Steam", &body), Kind::Unknown);
            stopped(decide(
                "Steam",
                &body,
                &buttons(&["Play anyway", "Cancel"]),
                true,
            ));
        }
    }

    #[test]
    fn explicit_title_accepts_actual_and_named_files_without_console() {
        for body in [ACTUAL, FILES] {
            assert_eq!(classify(TITLE, body), Kind::UnableToSync);
            assert_eq!(
                decide(TITLE, body, &buttons(&["Play anyway", "Cancel"]), false),
                Action::Select(0)
            );
        }
    }

    #[test]
    fn explicit_title_must_also_contain_steam() {
        assert_eq!(
            decide(
                "Sid Meier's Civilization VI",
                FILES,
                &buttons(&["Play anyway"]),
                true
            ),
            Action::None
        );
    }

    #[test]
    fn finite_english_and_japanese_name_allowlist() {
        for name in [
            "Sid Meier’s Civilization® VI",
            "Civilization VI",
            "シドマイヤーズ シヴィライゼーション VI",
            "シヴィライゼーションVI",
        ] {
            let title = format!("{name} - Steam");
            let body =
                format!("Steam was unable to sync your files for {name} with the Steam Cloud.");
            assert_eq!(
                decide(&title, &body, &buttons(&["Play anyway"]), false),
                Action::Select(0)
            );
        }
    }

    #[test]
    fn japanese_failure_and_button() {
        let body = "同期できません：SteamはシヴィライゼーションVIのファイルをSteamクラウドと同期できませんでした。";
        assert_eq!(
            classify("Steam - シヴィライゼーションVI", body),
            Kind::UnableToSync
        );
        assert_eq!(
            decide(
                "Steam - シヴィライゼーションVI",
                body,
                &buttons(&["キャンセル", "それでもプレイ"]),
                false
            ),
            Action::Select(1)
        );
    }

    #[test]
    fn conflict_selects_only_unique_pc_copy() {
        for safe in ["Upload to Steam Cloud", "Local files", "ローカルファイル"] {
            assert_eq!(classify(TITLE, CONFLICT), Kind::Conflict);
            assert_eq!(
                decide(
                    TITLE,
                    CONFLICT,
                    &buttons(&["Download to this machine", safe, "Cancel"]),
                    false
                ),
                Action::Select(1)
            );
        }
    }

    #[test]
    fn japanese_conflict_keeps_pc_files() {
        let body =
            "シヴィライゼーションVIのローカルファイルとSteamクラウドのファイルが競合しています。";
        assert_eq!(
            decide(
                "Steam - シヴィライゼーションVI",
                body,
                &buttons(&["ローカルファイル", "クラウドファイル"]),
                false
            ),
            Action::Select(0)
        );
    }

    #[test]
    fn download_only_is_never_selected() {
        for label in [
            "Download to this machine",
            "Download from Steam Cloud",
            "ダウンロード",
        ] {
            stopped(decide(TITLE, CONFLICT, &buttons(&[label]), true));
        }
    }

    #[test]
    fn conflicting_or_unknown_labels_cannot_masquerade_as_safe() {
        for label in [
            "Download Local files",
            "Local files / Download",
            "Upload to Steam Cloud (recommended)",
            "Not Local files",
        ] {
            stopped(decide(TITLE, CONFLICT, &buttons(&[label]), false));
        }
        stopped(decide(
            TITLE,
            CONFLICT,
            &buttons(&["Local files", "Keep these files"]),
            false,
        ));
    }

    #[test]
    fn duplicate_or_multiple_safe_candidates_stop() {
        for labels in [
            vec!["Local files", "Upload to Steam Cloud"],
            vec!["Local files", "ローカルファイル"],
            vec!["Local files", "Local files"],
        ] {
            stopped(decide(TITLE, CONFLICT, &buttons(&labels), false));
        }
        stopped(decide(
            TITLE,
            FILES,
            &buttons(&["Play anyway", "それでもプレイ"]),
            false,
        ));
    }

    #[test]
    fn no_buttons_or_safe_button_stops() {
        stopped(decide(TITLE, FILES, &[], false));
        stopped(decide(TITLE, FILES, &buttons(&["Cancel"]), false));
        stopped(decide("Steam", ACTUAL, &[], true));
    }

    #[test]
    fn syncing_waits_without_selecting_any_button() {
        for body in ["Syncing with Steam Cloud...", "Steamクラウドと同期中..."] {
            assert_eq!(classify(TITLE, body), Kind::Syncing);
            assert_eq!(
                decide(TITLE, body, &buttons(&["Play anyway", "Unknown"]), true),
                Action::Wait
            );
        }
    }

    #[test]
    fn offline_requires_explicit_civ6_title() {
        for body in ["Steam is in offline mode.", "Steamはオフラインモードです。"] {
            assert_eq!(classify(TITLE, body), Kind::Offline);
            assert_eq!(
                decide(TITLE, body, &buttons(&["Play anyway", "Cancel"]), false),
                Action::Select(0)
            );
            stopped(decide("Steam", body, &buttons(&["Play anyway"]), true));
        }
    }

    #[test]
    fn unknown_warning_stops_even_with_console_proof() {
        for title in [TITLE, "Steam"] {
            stopped(decide(
                title,
                "Unknown Steam Cloud error",
                &buttons(&["Play anyway"]),
                true,
            ));
            assert_eq!(classify(title, "Unknown Steam Cloud error"), Kind::Unknown);
        }
    }

    #[test]
    fn empty_body_is_rejected_for_dialogs() {
        for title in [TITLE, "Steam", "Steam - Unable to Sync"] {
            stopped(decide(title, " \n\t", &buttons(&["Play anyway"]), true));
            assert_eq!(classify(title, ""), Kind::Unknown);
        }
    }

    #[test]
    fn ordinary_steam_library_is_ignored() {
        for title in ["Steam", "Steam - Library", "Steam Library"] {
            for proof in [false, true] {
                assert_eq!(
                    decide(title, "Store Library Community", &[], proof),
                    Action::None
                );
                assert_eq!(decide(title, "", &[], proof), Action::None);
            }
        }
    }

    #[test]
    fn library_title_never_uses_console_override() {
        stopped(decide(
            "Steam - Library",
            ACTUAL,
            &buttons(&["Play anyway"]),
            true,
        ));
    }

    #[test]
    fn other_games_are_rejected_with_or_without_proof() {
        for game in [
            "Dota 2",
            "Civilization V",
            "Civilization VII",
            "Civilization VI Demo",
            "Unknown Future Game",
            "別のゲーム",
        ] {
            for proof in [false, true] {
                let other_title = format!("Steam - {game}");
                stopped(decide(
                    &other_title,
                    ACTUAL,
                    &buttons(&["Play anyway"]),
                    proof,
                ));
                let other_body =
                    format!("Steam was unable to sync your files for {game} with the Steam Cloud.");
                for title in [TITLE, "Steam"] {
                    stopped(decide(
                        title,
                        &other_body,
                        &buttons(&["Play anyway"]),
                        proof,
                    ));
                }
                for separator in [" and ", " / ", "\n"] {
                    let mixed_body = format!("{FILES}{separator}{game}");
                    stopped(decide(
                        TITLE,
                        &mixed_body,
                        &buttons(&["Play anyway"]),
                        proof,
                    ));
                }
            }
        }
    }

    #[test]
    fn mixed_title_does_not_pass_name_substring_matching() {
        for title in [
            "Steam - Civilization VI / Dota 2",
            "Steam - Dota 2 / Civilization VI",
            "NotSteam - Civilization VI",
            "Steam - Civilization VIII",
        ] {
            assert!(!matches!(
                decide(title, ACTUAL, &buttons(&["Play anyway"]), true),
                Action::Select(_)
            ));
        }
    }

    #[test]
    fn complete_generic_warning_is_not_a_substring_allowance() {
        for body in [
            format!("{ACTUAL}\nDota 2"),
            format!("Dota 2\n{ACTUAL}"),
            format!("{ACTUAL} Download your files."),
            ACTUAL.replace("permanently lose it", "keep it safely"),
        ] {
            assert_eq!(classify("Steam", &body), Kind::Unknown);
            stopped(decide("Steam", &body, &buttons(&["Play anyway"]), true));
        }
    }

    #[test]
    fn incomplete_stock_warning_is_not_console_authorized() {
        for body in ["Unable to Sync", "Steam was unable to sync your saves with the Steam Cloud.", "Unable to Sync: Steam was unable to sync your saves with the Steam Cloud; may permanently lose progress", FILES] {
            for proof in [false, true] {
                stopped(decide("Steam", body, &buttons(&["Play anyway"]), proof));
            }
        }
    }

    #[test]
    fn whitespace_case_and_original_button_index_are_preserved() {
        let body = ACTUAL.replace(' ', "\r\n  ");
        assert_eq!(
            decide(
                " STEAM ",
                &body,
                &buttons(&["Cancel", "Retry Sync", "  PLAY ANYWAY \n"]),
                true
            ),
            Action::Select(2)
        );
    }

    #[test]
    fn irrelevant_non_steam_window_is_ignored() {
        assert_eq!(
            decide("Browser", ACTUAL, &buttons(&["Play anyway"]), true),
            Action::None
        );
    }

    #[test]
    fn unknown_dialog_without_cloud_keywords_stops() {
        for title in ["Steam", "Steam - Unknown Warning"] {
            stopped(decide(
                title,
                "Unexpected failure",
                &buttons(&["Continue", "Cancel"]),
                true,
            ));
        }
        assert_eq!(
            decide(
                "Steam - Library",
                "Store Library Community",
                &buttons(&["Store", "Library", "Community"]),
                true
            ),
            Action::None
        );
    }

    #[test]
    fn measured_client_update_waits_without_game_proof_or_button_selection() {
        // Independently transcribed from live94 steam-warnings.jsonl.
        let body =
            "Updating Steam...\nUpdating Steam...\nVerifying installation...\nSystem\nSystem\n";
        for proof in [false, true] {
            assert_eq!(
                decide(
                    "Steam",
                    body,
                    &buttons(&["Cancel", "Minimize", "Close"]),
                    proof
                ),
                Action::Wait
            );
            for extra in ["\nCloud conflict", "\nDownload your saves", "\nDota 2"] {
                stopped(decide(
                    "Steam",
                    &format!("{body}{extra}"),
                    &buttons(&["Cancel", "Minimize", "Close"]),
                    proof,
                ));
            }
            stopped(decide(
                "Steam",
                body,
                &buttons(&["Cancel", "Minimize", "Close", "Play anyway"]),
                proof,
            ));
            stopped(decide(
                "Steam - Dota 2",
                body,
                &buttons(&["Cancel", "Minimize", "Close"]),
                proof,
            ));
        }
    }

    #[test]
    fn internal_template_token_is_not_a_game_name() {
        stopped(decide(
            TITLE,
            "Steam was unable to sync your files for {civ6} with the Steam Cloud.",
            &buttons(&["Play anyway"]),
            false,
        ));
    }

    #[test]
    fn every_selecting_kind_rejects_empty_or_ambiguous_buttons() {
        for body in [FILES, "Steam is in offline mode.", CONFLICT] {
            stopped(decide(TITLE, body, &buttons(&[""]), true));
            stopped(decide(
                TITLE,
                body,
                &buttons(&["Download from Steam Cloud"]),
                true,
            ));
        }
        stopped(decide(
            TITLE,
            "Steam is in offline mode.",
            &buttons(&["Play anyway", "Play anyway"]),
            true,
        ));
    }
}
