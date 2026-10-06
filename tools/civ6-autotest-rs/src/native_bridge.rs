//! Bounded native bridge controller. Its private joined mode can admit the
//! one pinned cost-one person; only a live receipt can prove that it works.
use crate::{Error, Result};
use std::path::{Path, PathBuf};

pub fn verify_controller_capabilities(success: bool, bytes: &[u8]) -> Result<()> {
    let actual = serde_json::from_slice::<serde_json::Value>(bytes).ok();
    if !success || actual != Some(civ6_native_bridge_rs::controller_capabilities()) {
        return Err(Error::input(
            "native bridge controller protocol differs from this runner; rebuild and pin the controller and DLL before another live run",
        ));
    }
    Ok(())
}

pub fn verify_override_hash(expected: Option<&str>, actual: &str) -> Result<()> {
    if actual.len() != 64
        || !actual.bytes().all(|b| b.is_ascii_hexdigit())
        || expected != Some(actual)
    {
        return Err(Error::input(
            "native bridge override does not match its frozen hashes",
        ));
    }
    Ok(())
}
#[cfg(test)]
mod asset_contract_tests {
    #[test]
    fn override_requires_both_an_explicit_and_matching_sha() {
        let sha = "a".repeat(64);
        super::verify_override_hash(Some(&sha), &sha).unwrap();
        for expected in [None, Some(""), Some("abc")] {
            assert!(super::verify_override_hash(expected, &sha).is_err());
        }
        assert!(super::verify_override_hash(Some(&"b".repeat(64)), &sha).is_err());
        assert!(super::verify_override_hash(Some(""), "").is_err());
    }
}

/// Installation/observation receipt only. It does not establish appointment,
/// payment, load safety or any successful gameplay decision.
pub fn validate_runtime_receipt(text: &str, pid: u32, created: u64) -> Result<()> {
    let rows: Vec<serde_json::Value> = text
        .lines()
        .map(serde_json::from_str)
        .collect::<std::result::Result<_, _>>()?;
    if rows.len() != 3
        || rows[0]["kind"] != "identity"
        || rows[0]["mode"] != "paid-assignment-runtime"
        || rows[0]["pid"] != pid
        || rows[0]["created_filetime"] != created
        || rows[0]["gamecore_sha256"] != civ6_native_bridge_rs::PINNED_GAMECORE_SHA256
        || rows[0]["runtime_lifetime"] != "process_exit"
        || rows[0]["reload_hook_observed"] != false
        || rows[0]["official_ai_payment_observed"] != false
        || !rows[0]["observation_limit_seconds"]
            .as_u64()
            .is_some_and(|n| (1..=300).contains(&n))
        || !rows[0]["bridge_sha256"]
            .as_str()
            .is_some_and(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
        || rows[1]["kind"] != "runtime_install"
        || rows[1]["status"] != 0
        || rows[1]["three_entries_published"] != true
        || !rows[1]["trace_address"]
            .as_u64()
            .is_some_and(|p| p != 0 && p & 7 == 0)
        || rows[1]["trace_owner"] != "game_process"
        || rows[1]["bounded_uninstall_allowed"] != false
        || rows[2]["kind"] != "observation_stop"
        || rows[2]["status"] != 0
        || rows[2]["runtime_retained"] != true
        || rows[2]["three_entries_published"] != true
        || rows[2]["trace_retained"] != true
    {
        return Err(Error::input(
            "resident runtime receipt is incomplete or differs",
        ));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssignmentSubject {
    pub owner: u32,
    pub governor: u32,
}
impl AssignmentSubject {
    pub fn parse(raw: &str) -> Result<Self> {
        let (owner, governor) = raw
            .split_once('|')
            .ok_or_else(|| Error::input("assignment subject owner/Index unavailable"))?;
        let subject = Self {
            owner: owner
                .parse()
                .map_err(|_| Error::input("invalid assignment subject owner"))?,
            governor: governor
                .parse()
                .map_err(|_| Error::input("invalid assignment subject Index"))?,
        };
        if subject.owner >= 64 || subject.governor >= 256 {
            return Err(Error::input("assignment subject exceeds native bounds"));
        }
        Ok(subject)
    }
}

pub fn assignment_subject_code(owner: u32, leader: &str) -> String {
    format!(
        r#"
local owner={owner}; local leader={leader}
local row=assert(GameInfo.JAC_LeaderPersonas[leader],'assignment subject definition missing')
local def=assert(GameInfo.Governors[row.GovernorType])
assert(Game:GetProperty('JAC_PERSONA_'..leader..'_OWNER')==owner,'assignment subject owner differs')
local gs=assert(Players[owner]):GetGovernors(); local gov=assert(gs:GetGovernor(def.Hash))
assert(type(def.Index)=='number' and def.Index%1==0 and def.Index>=0 and def.Index<256 and gov:GetType()==def.Index,'assignment subject identity differs')
return tostring(owner)..'|'..tostring(def.Index)
"#,
        leader = crate::scenario::lua_string(leader)
    )
}

#[cfg(windows)]
mod platform {
    use super::*;
    use civ6_native_bridge_rs::BridgeMode;
    use serde_json::Value;
    use sha2::{Digest, Sha256};
    use std::ffi::OsString;
    use std::io::{BufRead, BufReader, Read, Write};
    use std::os::windows::{ffi::OsStringExt, io::AsRawHandle, process::CommandExt};
    use std::process::{Child, ChildStdin, Command, Stdio};
    use std::sync::mpsc::{self, Receiver};
    use std::time::Duration;
    use windows_sys::Win32::Foundation::{
        CloseHandle, FILETIME, HANDLE, INVALID_HANDLE_VALUE, WAIT_OBJECT_0, WAIT_TIMEOUT,
    };
    use windows_sys::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Module32FirstW, Module32NextW, MODULEENTRY32W, TH32CS_SNAPMODULE,
        TH32CS_SNAPMODULE32,
    };
    use windows_sys::Win32::System::Threading::{
        GetProcessTimes, OpenProcess, QueryFullProcessImageNameW, TerminateProcess,
        WaitForSingleObject, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
        PROCESS_TERMINATE,
    };

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    fn wait_capability_child(
        child: &mut Child,
        budget: Duration,
    ) -> Result<std::process::ExitStatus> {
        let started = std::time::Instant::now();
        loop {
            match child.try_wait() {
                Ok(Some(status)) => return Ok(status),
                Ok(None) if started.elapsed() < budget => {
                    std::thread::sleep(Duration::from_millis(10));
                }
                outcome => {
                    let killed = child.kill();
                    let reaped = child.wait();
                    return Err(Error::input(format!(
                        "native bridge capability query did not finish within its budget: {outcome:?}; child kill={killed:?}; child reap={reaped:?}"
                    )));
                }
            }
        }
    }

    fn query_capabilities(executable: &Path) -> Result<()> {
        let mut child = Command::new(executable)
            .arg("--capabilities")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()?;
        let status = wait_capability_child(&mut child, Duration::from_secs(3))?;
        let bytes = match child.stdout.take() {
            Some(mut stdout) => read_capability_response(&mut stdout)?,
            None => Vec::new(),
        };
        verify_controller_capabilities(status.success(), &bytes)
    }

    /// The controller has exited. Consume only bytes already queued, never EOF:
    /// an inherited writer can stay open without delaying this negotiation.
    fn read_capability_response(pipe: &mut (impl Read + AsRawHandle)) -> Result<Vec<u8>> {
        let mut available = 0;
        let ok = unsafe {
            windows_sys::Win32::System::Pipes::PeekNamedPipe(
                pipe.as_raw_handle().cast(),
                std::ptr::null_mut(),
                0,
                std::ptr::null_mut(),
                &mut available,
                std::ptr::null_mut(),
            )
        };
        if ok == 0 {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() == Some(109) {
                return Ok(Vec::new());
            }
            return Err(error.into());
        }
        if available > 16_384 {
            return Err(Error::input(
                "native bridge capability response is too large",
            ));
        }
        let mut bytes = vec![0; available as usize];
        pipe.read_exact(&mut bytes)?;
        Ok(bytes)
    }

    fn hex_address(value: &Value) -> Option<u64> {
        let raw = value.as_str()?.strip_prefix("0x")?;
        (raw.len() == 16)
            .then(|| u64::from_str_radix(raw, 16).ok())
            .flatten()
    }

    fn valid_promotion_snapshot(row: &Value) -> bool {
        let Some(status) = row["promotion_snapshot_status"]
            .as_u64()
            .filter(|n| *n <= 7)
        else {
            return false;
        };
        let Some(count) = row["promotion_count"]
            .as_u64()
            .filter(|n| *n <= civ6_native_bridge_rs::governor_payment::PROMOTION_LIMIT as u64)
        else {
            return false;
        };
        let Some(indices) = row["owned_promotion_indices"].as_array() else {
            return false;
        };
        if row["payment_authority"] != false {
            return false;
        }
        if status != 0 {
            return count == 0
                && indices.is_empty()
                && row["promotion_owner"] == u32::MAX
                && row["promotion_index"] == u32::MAX;
        }
        if count == 0
            || row["promotion_owner"] != row["player_id"]
            || row["promotion_index"] != row["governor_index"]
            || row["promotion_owner"].as_u64().is_none_or(|n| n >= 64)
            || row["promotion_index"].as_u64().is_none_or(|n| n >= 256)
            || row["read_flags"].as_u64().is_none_or(|n| n & 3 != 3)
        {
            return false;
        }
        let mut previous = None;
        for index in indices {
            let Some(index) = index
                .as_u64()
                .filter(|n| *n < count && previous.is_none_or(|old| old < *n))
            else {
                return false;
            };
            previous = Some(index);
        }
        true
    }

    #[cfg(test)]
    fn terminal_artifact(path: &Path, pid: u32, created: u64, mode: BridgeMode) -> Result<usize> {
        terminal_artifact_for_subject(path, pid, created, mode, None)
    }

    fn terminal_artifact_for_subject(
        path: &Path,
        pid: u32,
        created: u64,
        mode: BridgeMode,
        subject: Option<AssignmentSubject>,
    ) -> Result<usize> {
        if subject.is_some_and(|s| s.owner >= 64 || s.governor >= 256)
            || (subject.is_some() && mode != BridgeMode::AssignCalibration)
        {
            return Err(Error::input(
                "invalid bounded assignment observation subject",
            ));
        }
        let subject = subject.unwrap_or(AssignmentSubject {
            owner: 0,
            governor: 4,
        });
        let dual = matches!(
            mode,
            BridgeMode::JoinedAdmission | BridgeMode::ProductionAdmission
        );
        let mut identity = 0usize;
        let mut installed = 0usize;
        let mut restored = 0usize;
        let mut stopped = 0usize;
        let mut entries = 0usize;
        let mut assign_ui = std::collections::BTreeSet::new();
        let mut assign_update = std::collections::BTreeSet::new();
        let mut joined_calibrations = 0usize;
        let mut joined_properties = 0usize;
        let mut joined_positive = false;
        let mut joined_positive_receivers = std::collections::BTreeSet::new();
        let mut joined_true_property_receivers = std::collections::BTreeSet::new();
        let mut scoped_keys = std::collections::BTreeSet::new();
        let mut partial_dropped = None;
        let mut production_bindings = std::collections::BTreeSet::new();
        let mut production_resets = std::collections::BTreeSet::new();
        let mut calibrated_owners = std::collections::BTreeSet::new();
        let mut admission_values = std::collections::BTreeSet::new();
        let mut admission_threads = std::collections::BTreeSet::new();
        let mut admission_true_after_false = false;
        for line in BufReader::new(std::fs::File::open(path)?).lines() {
            let row: Value = serde_json::from_str(&line?)?;
            if stopped != 0 {
                return Err(Error::input(
                    "native bridge wrote after its terminal record",
                ));
            }
            match row["kind"].as_str() {
                Some("identity")
                    if row["pid"] == pid
                        && row["created_filetime"] == created
                        && row["mode"]
                            == match mode {
                                BridgeMode::CanEver => "can-ever",
                                BridgeMode::Property => "property",
                                BridgeMode::PropertyScoped => "property-scoped",
                                BridgeMode::CanEverCalibration => "can-ever-calibration",
                                BridgeMode::PropertyAdmissionScoped => "property-admission-scoped",
                                BridgeMode::JoinedAdmission => "joined-admission",
                                BridgeMode::ProductionAdmission => "production-admission",
                                BridgeMode::PromotionCall => "promotion-call",
                                BridgeMode::AssignCalibration => "assign-calibration",
                            }
                        && row["entry_rva"] == mode.rva()
                        && (matches!(
                            mode,
                            BridgeMode::CanEver | BridgeMode::CanEverCalibration | BridgeMode::PromotionCall | BridgeMode::AssignCalibration
                        ) || row["prefix_bytes"] == "[cc, cc, cc, cc, cc]")
                        && (mode != BridgeMode::PromotionCall
                            || (row["original_callee_rva"] == civ6_native_bridge_rs::PINNED_PROMOTION_CALLEE_RVA
                                && row["original_return_site_rva"] == civ6_native_bridge_rs::PINNED_PROMOTION_CALL_RVA + 5
                                && row["observation_scope"] == "one_official_promote_call_entry_only"
                                && row["call_window"] == "[e8, 9c, ef, fc, ff, 84, c0, 0f, 84, 75, 01, 00, 00, 44, 8b, c7]"))
                        && (mode != BridgeMode::JoinedAdmission
                            || (row["property_rva"]
                                == civ6_native_bridge_rs::PINNED_PROPERTY_GET_RVA
                                && row["scope_key_ids"]
                                    == serde_json::json!([
                                        civ6_native_bridge_rs::ASHINA_ADMISSION_KEY_ID
                                    ])))
                        && (mode != BridgeMode::ProductionAdmission
                            || (row["property_rva"]
                                == civ6_native_bridge_rs::PINNED_PROPERTY_GET_RVA
                                && row["scope_key_ranges"]
                                    == serde_json::json!([
                                        [900000, 900255],
                                        [901000, 901255],
                                        [903000, 903255]
                                    ])
                                && row["reset_key_range"]
                                    == serde_json::json!([
                                        civ6_native_bridge_rs::PRODUCTION_RESET_KEY,
                                        civ6_native_bridge_rs::PRODUCTION_RESET_LAST_KEY
                                    ])))
                        && (mode != BridgeMode::PropertyScoped
                            || row["scope_key_ids"]
                                == serde_json::json!([
                                    civ6_native_bridge_rs::PROPERTY_TEST_VALUE_KEY_ID,
                                    civ6_native_bridge_rs::PROPERTY_TEST_PHASE_KEY_ID,
                                ]))
                        && (mode != BridgeMode::PropertyAdmissionScoped
                            || row["scope_key_ids"]
                                == serde_json::json!([
                                    civ6_native_bridge_rs::ASHINA_ADMISSION_KEY_ID,
                                ]))
                        && row["gamecore_sha256"]
                            == civ6_native_bridge_rs::PINNED_GAMECORE_SHA256 =>
                {
                    identity += 1
                }
                Some("install") if row["status"] == 0 => installed += 1,
                Some("uninstall")
                    if row["status"] == 0
                        && row["entry_restored"] == true
                        && row["prefix_retained_until_exit"] == (mode.is_property() || dual) =>
                {
                    restored += 1
                }
                Some("entry")
                    if mode == BridgeMode::CanEver
                        && row["sequence"].as_u64() == Some(entries as u64 + 1) =>
                {
                    entries += 1
                }
                Some("promotion_call_entry")
                    if mode == BridgeMode::PromotionCall
                        && row["sequence"].as_u64() == Some(entries as u64 + 1)
                        && row["thread_id"].as_u64().is_some_and(|id| id > 0)
                        && ["raw_rcx", "raw_rdx", "raw_r8", "raw_r9"]
                            .into_iter().all(|name| hex_address(&row[name]).is_some())
                        && row["return_site_rva"] == civ6_native_bridge_rs::PINNED_PROMOTION_CALL_RVA + 5
                        && row["entry_only"] == true
                        && row.get("original_return_value").is_none() =>
                {
                    entries += 1
                }
                Some("calibration")
                    if matches!(
                        mode,
                        BridgeMode::CanEverCalibration
                            | BridgeMode::JoinedAdmission
                            | BridgeMode::ProductionAdmission
                    ) && row["sequence"].as_u64()
                        == Some(if dual {
                            joined_calibrations as u64 + 1
                        } else {
                            entries as u64 + 1
                        })
                        && row["thread_id"].as_u64().is_some_and(|id| id > 0)
                        && row["owner_read_state"] == 1
                        && hex_address(&row["governors_this"]).is_some_and(|p| p != 0)
                        && hex_address(&row["player_address"])
                            .and_then(|player| player.checked_add(0x528))
                            == hex_address(&row["property_receiver"])
                        && row["original_al"].as_u64().is_some_and(|n| n <= 1)
                        && (!dual || row["effective_al"].as_u64().is_some_and(|n| n <= 1)) =>
                {
                    if mode == BridgeMode::ProductionAdmission {
                        if let (Some(owner), Some(receiver)) = (
                            row["player_id"].as_u64(),
                            hex_address(&row["property_receiver"]),
                        ) {
                            production_bindings.insert((owner, receiver));
                        }
                    }
                    if dual {
                        joined_calibrations += 1;
                        if row["player_id"] == 0
                            && row["governor_index"] == 4
                            && row["original_al"] == 0
                            && row["effective_al"] == 1
                        {
                            joined_positive = true;
                            if let Some(receiver) = hex_address(&row["property_receiver"]) {
                                joined_positive_receivers.insert(receiver);
                            }
                        }
                    }
                    if row["governor_index"] == 4
                        && row["return_site_rva"] == 0xba523
                        && row["original_al"] == 0
                    {
                        if let Some(owner) = row["player_id"].as_u64() {
                            calibrated_owners.insert(owner);
                        }
                    }
                    entries += 1
                }
                Some("assign_calibration")
                    if mode == BridgeMode::AssignCalibration
                        && row["sequence"].as_u64() == Some(entries as u64 + 1)
                        && row["thread_id"].as_u64().is_some_and(|n| n > 0)
                        && row["read_flags"].as_u64().is_some_and(|n| n <= 7)
                        && [
                            "governors_this",
                            "governor_pointer",
                            "city_pointer",
                            "raw_r9",
                            "player_address",
                            "original_rax",
                        ]
                        .into_iter()
                        .all(|key| hex_address(&row[key]).is_some())
                        && row["original_al"].as_u64().is_some_and(|n| n <= 255)
                        && row["effective_al"] == row["original_al"]
                        && hex_address(&row["original_rax"]).map(|n| n & 255)
                            == row["original_al"].as_u64()
                        && [
                            "player_id",
                            "governor_index",
                            "city_a8",
                            "city_c0",
                            "city_c4",
                            "city_d8",
                        ]
                        .into_iter()
                        .all(|key| row[key].as_u64().is_some_and(|n| n <= u32::MAX as u64)) =>
                {
                    if !valid_promotion_snapshot(&row) {
                        return Err(Error::input("invalid read-only native promotion snapshot"));
                    }
                    if row["read_flags"] == 7
                        && row["player_id"] == subject.owner
                        && row["governor_index"] == subject.governor
                        && row["city_d8"] == subject.owner
                        && row["original_al"].as_u64().is_some_and(|n| n <= 1)
                        && [
                            "governors_this",
                            "governor_pointer",
                            "city_pointer",
                            "player_address",
                        ]
                        .into_iter()
                        .all(|key| hex_address(&row[key]).is_some_and(|n| n != 0))
                    {
                        let pair = (
                            hex_address(&row["governors_this"]),
                            hex_address(&row["governor_pointer"]),
                            hex_address(&row["city_pointer"]),
                            hex_address(&row["player_address"]),
                            row["city_a8"].as_u64(),
                            row["city_c0"].as_u64(),
                            row["city_c4"].as_u64(),
                        );
                        if row["return_site_rva"] == 0xba3e2 {
                            assign_ui.insert(pair);
                        }
                        if row["return_site_rva"] == 0x2dd375
                            && row["raw_r9"] == "0x0000000000000000"
                        {
                            assign_update.insert(pair);
                        }
                    }
                    entries += 1;
                }
                Some("property_get")
                    if (mode.is_property() || dual)
                        && row["sequence"].as_u64()
                            == Some(if dual {
                                joined_properties as u64 + 1
                            } else {
                                entries as u64 + 1
                            })
                        && row["property"]
                            .as_str()
                            .is_some_and(|s| s.starts_with("0x") && s.len() == 18)
                        && row["returned"]
                            .as_str()
                            .is_some_and(|s| s.starts_with("0x") && s.len() == 18)
                        && row["key_id"].as_u64().is_some_and(|n| {
                            n <= u32::MAX as u64
                                && (mode != BridgeMode::PropertyScoped
                                    || n == u64::from(
                                        civ6_native_bridge_rs::PROPERTY_TEST_VALUE_KEY_ID,
                                    )
                                    || n == u64::from(
                                        civ6_native_bridge_rs::PROPERTY_TEST_PHASE_KEY_ID,
                                    ))
                                && (!matches!(
                                    mode,
                                    BridgeMode::PropertyAdmissionScoped
                                        | BridgeMode::JoinedAdmission
                                ) || n
                                    == u64::from(civ6_native_bridge_rs::ASHINA_ADMISSION_KEY_ID))
                                && (mode != BridgeMode::ProductionAdmission
                                    || (u64::from(civ6_native_bridge_rs::PRODUCTION_RESET_KEY)
                                        ..=u64::from(civ6_native_bridge_rs::PRODUCTION_RESET_LAST_KEY))
                                        .contains(&n)
                                    || (900000..900256).contains(&n)
                                    || (901000..901256).contains(&n)
                                    || (903000..903256).contains(&n))
                        })
                        && row["thread_id"]
                            .as_u64()
                            .is_some_and(|n| n > 0 && n <= u32::MAX as u64)
                        && row["read_state"].as_u64().is_some_and(|n| n <= 2) =>
                {
                    let returned = row["returned"].as_str().unwrap();
                    if mode == BridgeMode::PropertyScoped {
                        scoped_keys.insert(row["key_id"].as_u64().unwrap());
                    } else if matches!(
                        mode,
                        BridgeMode::PropertyAdmissionScoped | BridgeMode::JoinedAdmission
                    ) {
                        scoped_keys.insert(row["key_id"].as_u64().unwrap());
                        if row["read_state"] != 1
                            || row["tag"] != 1
                            || row["flags14"] != 0
                            || !matches!(
                                row["payload8"].as_str(),
                                Some("0x0000000000000000" | "0x0000000000000001")
                            )
                        {
                            return Err(Error::input(
                                "admission property was not a copied native bool",
                            ));
                        }
                        admission_values.insert(row["payload8"].as_str().unwrap().to_owned());
                        admission_threads.insert(row["thread_id"].as_u64().unwrap());
                        if row["payload8"] == "0x0000000000000001"
                            && admission_values.contains("0x0000000000000000")
                        {
                            admission_true_after_false = true;
                        }
                        if mode == BridgeMode::JoinedAdmission
                            && row["payload8"] == "0x0000000000000001"
                        {
                            if let Some(receiver) = hex_address(&row["property"]) {
                                joined_true_property_receivers.insert(receiver);
                            }
                        }
                    }
                    if row["read_state"] == 0 {
                        if returned != "0x0000000000000000"
                            || !row["tag"].is_null()
                            || !row["payload8"].is_null()
                            || !row["flags14"].is_null()
                        {
                            return Err(Error::input("invalid missing property row"));
                        }
                    } else if row["read_state"] == 1 {
                        if returned == "0x0000000000000000"
                            || row["tag"].as_u64().is_none_or(|n| n > u16::MAX as u64)
                            || !row["payload8"]
                                .as_str()
                                .is_some_and(|s| s.starts_with("0x") && s.len() == 18)
                            || row["flags14"]
                                .as_u64().is_none_or(|n| n > u32::MAX as u64)
                        {
                            return Err(Error::input("invalid copied property row"));
                        }
                    } else if returned == "0x0000000000000000"
                        || !row["tag"].is_null()
                        || !row["payload8"].is_null()
                        || !row["flags14"].is_null()
                    {
                        return Err(Error::input("invalid unreadable property row"));
                    }
                    if mode == BridgeMode::ProductionAdmission {
                        let key = row["key_id"].as_u64().unwrap();
                        if (u64::from(civ6_native_bridge_rs::PRODUCTION_RESET_KEY)
                            ..=u64::from(civ6_native_bridge_rs::PRODUCTION_RESET_LAST_KEY))
                            .contains(&key)
                            && row["read_state"] == 1
                            && row["tag"] == 1
                            && row["flags14"] == 0
                            && row["payload8"] == "0x0000000000000001"
                        {
                            if let Some(receiver) = hex_address(&row["property"]) {
                                production_resets.insert((
                                    key - u64::from(civ6_native_bridge_rs::PRODUCTION_RESET_KEY),
                                    receiver,
                                ));
                            }
                        }
                        let cell_index = if (900000..900256).contains(&key) {
                            Some(key - 900000)
                        } else if (901000..901256).contains(&key) {
                            Some(key - 901000)
                        } else {
                            None
                        };
                        let malformed = row["read_state"] == 1
                            && (row["tag"] != 1
                                || row["flags14"] != 0
                                || !matches!(
                                    row["payload8"].as_str(),
                                    Some("0x0000000000000000" | "0x0000000000000001")
                                ));
                        if let Some(index) = cell_index.filter(|_| malformed) {
                            // Captured immediately after applying this raw observation,
                            // by the native consumer for this same receiver/owner/index.
                            if row["denial_checked"] != true
                                || row["denial_index"] != index
                                || !row["denial_owner"].as_u64().is_some_and(|owner| {
                                    owner < 64
                                        && hex_address(&row["property"]).is_some_and(|receiver| {
                                            production_bindings.contains(&(owner, receiver))
                                        })
                                })
                            {
                                return Err(Error::input("malformed production property lacks same-cell native closure proof"));
                            }
                        }
                    }
                    if dual {
                        joined_properties += 1;
                    }
                    entries += 1;
                }
                Some("stop")
                    if row["rows"] == entries
                        && row["in_flight"] == 0
                        && row["snapshot_stable"] == true
                        && (!matches!(mode, BridgeMode::PromotionCall | BridgeMode::AssignCalibration)
                            || (row["quiescence_proven"] == false
                                && row["counter_scope"] == "snapshot_observation_not_complete_call_history"))
                        && row["observed_calls"].as_u64().is_some_and(|n| {
                            n == entries as u64 + row["dropped"].as_u64().unwrap_or(u64::MAX)
                        })
                        && (if matches!(
                            mode,
                            BridgeMode::CanEver
                                | BridgeMode::CanEverCalibration
                                | BridgeMode::JoinedAdmission
                                | BridgeMode::ProductionAdmission
                                | BridgeMode::PromotionCall
                                | BridgeMode::AssignCalibration
                        ) {
                            row["result"] == "ok"
                                && (row["dropped"] == 0
                                    || (mode == BridgeMode::ProductionAdmission
                                        && row["counter_scope"]
                                            == "bounded_prefix_samples_not_complete_call_history"
                                        && row["calibration_sample_limit"] == 1024
                                        && row["property_sample_limit"] == 4096))
                                && row["classification"]
                                    == if mode == BridgeMode::CanEver {
                                        "can_ever_entry"
                                    } else if mode == BridgeMode::AssignCalibration {
                                        "assign_calibration"
                                    } else if mode == BridgeMode::JoinedAdmission {
                                        "joined_admission"
                                    } else if mode == BridgeMode::ProductionAdmission {
                                        "production_admission"
                                    } else if mode == BridgeMode::PromotionCall {
                                        "promotion_call_entry_only"
                                    } else {
                                        "can_ever_calibration"
                                    }
                                && row["prefix_retained_until_exit"] == dual
                        } else {
                            row["classification"] == "raw_collection"
                                && row["prefix_retained_until_exit"] == true
                                && row["quiescence_proven"] == false
                                && (!matches!(
                                    mode,
                                    BridgeMode::PropertyScoped
                                        | BridgeMode::PropertyAdmissionScoped
                                ) || row["filtered_calls"].as_u64().is_some_and(|filtered| {
                                    row["getter_calls_seen"].as_u64()
                                        == Some(
                                            filtered
                                                + row["observed_calls"]
                                                    .as_u64()
                                                    .unwrap_or(u64::MAX),
                                        )
                                }))
                                && row["dropped"]
                                    .as_u64()
                                    .is_some_and(|n| n <= u32::MAX as u64)
                                && (if row["dropped"] == 0 {
                                    row["result"] == "raw_collected"
                                } else {
                                    row["result"] == "partial_raw"
                                })
                        }) =>
                {
                    if mode.is_property() {
                        partial_dropped = row["dropped"].as_u64();
                    }
                    stopped += 1
                }
                _ => {
                    return Err(Error::input(
                        "native bridge artifact has an invalid proof row",
                    ))
                }
            }
        }
        if identity != 1 || installed != 1 || restored != 1 || stopped != 1 || entries == 0 {
            return Err(Error::input(
                "native bridge terminal proof or caller evidence missing",
            ));
        }
        if let Some(dropped) = partial_dropped.filter(|n| *n > 0) {
            return Err(Error::input(format!(
                "native property raw collection is partial: {entries} rows, {dropped} dropped; inspect {}",
                path.display()
            )));
        }
        if mode == BridgeMode::AssignCalibration && assign_ui.is_disjoint(&assign_update) {
            return Err(Error::input(
                "native assignment UI/update pointer ABI evidence missing",
            ));
        }
        if mode == BridgeMode::PropertyScoped
            && scoped_keys
                != std::collections::BTreeSet::from([
                    u64::from(civ6_native_bridge_rs::PROPERTY_TEST_VALUE_KEY_ID),
                    u64::from(civ6_native_bridge_rs::PROPERTY_TEST_PHASE_KEY_ID),
                ])
        {
            return Err(Error::input(
                "native property collection did not observe both scoped keys",
            ));
        }
        if mode == BridgeMode::PropertyAdmissionScoped
            && scoped_keys
                != std::collections::BTreeSet::from([u64::from(
                    civ6_native_bridge_rs::ASHINA_ADMISSION_KEY_ID,
                )])
        {
            return Err(Error::input(
                "native admission property key was not observed",
            ));
        }
        if mode == BridgeMode::CanEverCalibration
            && ![0, 1]
                .into_iter()
                .all(|owner| calibrated_owners.contains(&owner))
        {
            return Err(Error::input(
                "native owner 0/1 original-result calibration missing",
            ));
        }
        if mode == BridgeMode::PropertyAdmissionScoped
            && (admission_values.len() != 2
                || admission_threads.len() < 2
                || !admission_true_after_false)
        {
            return Err(Error::input(
                "native admission false/true or Core/UI threads missing",
            ));
        }
        if mode == BridgeMode::JoinedAdmission
            && (!joined_positive
                || joined_calibrations == 0
                || joined_properties == 0
                || !admission_true_after_false
                || admission_threads.len() < 2
                || joined_positive_receivers.is_disjoint(&joined_true_property_receivers))
        {
            return Err(Error::input(
                "joined native positive is not bound to a Core getter true",
            ));
        }
        if mode == BridgeMode::ProductionAdmission
            && (joined_calibrations == 0
                || joined_properties == 0
                || production_bindings.is_disjoint(&production_resets))
        {
            return Err(Error::input(
                "production admission lacks a sampled owner reset paired with Core getter",
            ));
        }
        Ok(entries)
    }

    struct PinnedTarget(HANDLE);
    // Kernel process/snapshot handles are usable from any thread; this wrapper
    // owns the sole CloseHandle and never exposes a borrowed raw handle.
    unsafe impl Send for PinnedTarget {}
    impl Drop for PinnedTarget {
        fn drop(&mut self) {
            unsafe { CloseHandle(self.0) };
        }
    }

    fn same_file(actual: &Path, expected: &str) -> bool {
        actual
            .canonicalize()
            .ok()
            .zip(Path::new(expected).canonicalize().ok())
            .is_some_and(|(a, b)| {
                a.to_string_lossy()
                    .eq_ignore_ascii_case(&b.to_string_lossy())
            })
    }

    fn pinned_created(pid: u32, expected: f64) -> Result<(u64, PinnedTarget)> {
        let handle = unsafe {
            OpenProcess(
                PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE | PROCESS_TERMINATE,
                0,
                pid,
            )
        };
        if handle.is_null() {
            return Err(Error::input("native bridge game process unavailable"));
        }
        let handle = PinnedTarget(handle);
        let mut a = FILETIME::default();
        let mut b = FILETIME::default();
        let mut c = FILETIME::default();
        let mut d = FILETIME::default();
        let ok = unsafe { GetProcessTimes(handle.0, &mut a, &mut b, &mut c, &mut d) } != 0;
        let mut image = vec![0u16; 32768];
        let mut len = image.len() as u32;
        let path_ok =
            unsafe { QueryFullProcessImageNameW(handle.0, 0, image.as_mut_ptr(), &mut len) } != 0;
        if !ok || !path_ok || len == 0 || len as usize >= image.len() {
            return Err(Error::input("native bridge game identity unreadable"));
        }
        let created = (u64::from(a.dwHighDateTime) << 32) | u64::from(a.dwLowDateTime);
        let unix = created as f64 / 10_000_000.0 - 11_644_473_600.0;
        let path = PathBuf::from(OsString::from_wide(&image[..len as usize]));
        if unix != expected || !same_file(&path, civ6_native_bridge_rs::GAME_EXE) {
            return Err(Error::input("native bridge pinned game identity changed"));
        }
        Ok((created, handle))
    }

    fn pinned_module(pid: u32) -> Result<()> {
        let snapshot =
            unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPMODULE | TH32CS_SNAPMODULE32, pid) };
        if snapshot.is_null() || snapshot == INVALID_HANDLE_VALUE {
            return Err(Error::input("pinned GameCore module snapshot unavailable"));
        }
        let snapshot = PinnedTarget(snapshot);
        let mut entry: MODULEENTRY32W = unsafe { std::mem::zeroed() };
        entry.dwSize = std::mem::size_of::<MODULEENTRY32W>() as u32;
        if unsafe { Module32FirstW(snapshot.0, &mut entry) } == 0 {
            return Err(Error::input("pinned GameCore module list unavailable"));
        }
        let found = loop {
            let end = entry
                .szExePath
                .iter()
                .position(|c| *c == 0)
                .unwrap_or(entry.szExePath.len());
            if same_file(
                &PathBuf::from(OsString::from_wide(&entry.szExePath[..end])),
                civ6_native_bridge_rs::GAMECORE,
            ) && entry.modBaseSize as usize > civ6_native_bridge_rs::PINNED_CAN_EVER_RVA + 8
            {
                break true;
            }
            if unsafe { Module32NextW(snapshot.0, &mut entry) } == 0 {
                break false;
            }
        };
        if !found {
            return Err(Error::input("pinned GameCore module changed before close"));
        }
        let bytes = std::fs::read(civ6_native_bridge_rs::GAMECORE)?;
        if format!("{:x}", Sha256::digest(bytes)) != civ6_native_bridge_rs::PINNED_GAMECORE_SHA256 {
            return Err(Error::input("pinned GameCore hash changed before close"));
        }
        Ok(())
    }

    fn finish_failed_cleanup(
        closure: Result<()>,
        target: &PinnedTarget,
        controller: &mut Child,
    ) -> Result<()> {
        if unsafe { WaitForSingleObject(target.0, 0) } != WAIT_OBJECT_0 {
            return Err(Error::input(format!(
                "pinned target is still running; controller retained; close={closure:?}"
            )));
        }
        if controller.try_wait()?.is_none() {
            controller.kill()?;
            controller.wait()?;
        }
        closure
    }

    /// Close only the PID with the exact creation FILETIME, DX11 image and pinned XP2 module.
    pub fn close_pinned_game(pid: u32, started: f64) -> Result<()> {
        let (created, inspected) = pinned_created(pid, started)?;
        if unsafe { WaitForSingleObject(inspected.0, 0) } == WAIT_OBJECT_0 {
            return Ok(());
        }
        pinned_module(pid)?;
        let mut a = FILETIME::default();
        let mut b = FILETIME::default();
        let mut c = FILETIME::default();
        let mut d = FILETIME::default();
        if unsafe { GetProcessTimes(inspected.0, &mut a, &mut b, &mut c, &mut d) } == 0
            || ((u64::from(a.dwHighDateTime) << 32) | u64::from(a.dwLowDateTime)) != created
        {
            return Err(Error::input(
                "pinned game creation time changed before close",
            ));
        }
        if unsafe { TerminateProcess(inspected.0, 1) } == 0
            && unsafe { WaitForSingleObject(inspected.0, 0) } != WAIT_OBJECT_0
        {
            return Err(Error::input("pinned game termination failed"));
        }
        if unsafe { WaitForSingleObject(inspected.0, 10_000) } != WAIT_OBJECT_0 {
            return Err(Error::input("pinned game exit not confirmed"));
        }
        Ok(())
    }

    pub struct Session {
        child: Child,
        input: Option<ChildStdin>,
        lines: Receiver<String>,
        artifact: PathBuf,
        pid: u32,
        created: u64,
        started: f64,
        target: PinnedTarget,
        close_game: Box<dyn FnMut(u32, f64) -> Result<()> + Send>,
        finished: bool,
        mode: BridgeMode,
        assignment_subject: Option<AssignmentSubject>,
        resident_runtime: bool,
        ai_diagnostic: civ6_native_bridge_rs::ai_diagnostic::Session,
        reader_requested:bool,
        reader_stop:Option<Value>,
        _binary_pins: (std::fs::File, std::fs::File),
    }

    impl Session {
        pub fn ai_diagnostic(&mut self, request: &Value) -> Result<Value> {
            if request["action"]=="start_native_ai_reader" {
                if self.reader_requested || !self.ai_diagnostic.unused() || self.finished || !self.resident_runtime || self.process_has_exited() {
                    return Err(Error::input("reader registration already requested or resident absent"));
                }
                civ6_native_bridge_rs::ai_reader::candidates(request).map_err(Error::input)?;
                self.reader_requested=true;
                let receipt=self.ai_exchange(request)?;
                let actual=civ6_native_bridge_rs::ai_reader::selected(request,&receipt,self.pid,self.created).map_err(Error::input)?;
                let command=civ6_native_bridge_rs::ai_diagnostic::Command::parse(&actual,self.pid,self.created).map_err(Error::input)?;
                self.ai_diagnostic.reserve(command).map_err(Error::input)?;
                let mut stop=actual;stop["action"]="stop_native_ai_observation".into();self.reader_stop=Some(stop);
                return Ok(receipt);
            }
            if request["action"]=="stop_native_ai_reader" {
                let stop=self.reader_stop.take().ok_or_else(||Error::input("reader actual request unknown or stop already sent"))?;
                return self.ai_diagnostic(&stop);
            }
            if self.reader_requested && request["action"]=="start_native_ai_observation" {return Err(Error::input("reader request cannot be replaced"));}
            use civ6_native_bridge_rs::ai_diagnostic::{Command,validate_matched_receipt};
            if !self.resident_runtime || self.finished || self.process_has_exited() {
                return Err(Error::input("AI diagnostics require the active resident controller"));
            }
            let command=Command::parse(request,self.pid,self.created).map_err(Error::input)?;
            self.ai_diagnostic.reserve(command).map_err(Error::input)?;
            let value=self.ai_exchange(request)?;
            validate_matched_receipt(&value,command).map_err(Error::input)?;
            Ok(value)
        }
        fn ai_exchange(&mut self,request:&Value)->Result<Value> {
            let input=self.input.as_mut().ok_or_else(||Error::input("AI controller input is closed"))?;
            writeln!(input,"{request}").and_then(|_|input.flush()).map_err(|e|Error::input(e.to_string()))?;
            let line=self.lines.recv_timeout(Duration::from_secs(25))
                .map_err(|_|Error::input("AI diagnostic response unknown; no retry"))?;
            let value:Value=serde_json::from_str(line.strip_prefix("AI_RECEIPT ")
                .ok_or_else(||Error::input("AI controller response differs; no retry"))?)
                .map_err(|e|Error::input(e.to_string()))?;
            Ok(value)
        }
        pub fn process_identity(&self) -> (u32, u64) {
            (self.pid, self.created)
        }
        pub fn retains_process_after_stop(&self) -> bool {
            self.resident_runtime && self.finished
        }
        pub fn process_has_exited(&self) -> bool {
            unsafe { WaitForSingleObject(self.target.0, 0) == WAIT_OBJECT_0 }
        }
        pub fn verify_current_process(&self, pid: u32, started: f64) -> Result<(u32, u64)> {
            let (created, target) = pinned_created(pid, started)?;
            if (self.finished && !self.resident_runtime)
                || (pid, created) != (self.pid, self.created)
                || unsafe { WaitForSingleObject(target.0, 0) } != WAIT_TIMEOUT
            {
                return Err(Error::input(
                    "UI process differs from the active native observation",
                ));
            }
            Ok((pid, created))
        }
        pub fn start(
            pid: u32,
            started: f64,
            output: &Path,
            name: &str,
            seconds: u64,
            mode: &str,
            close_game: Box<dyn FnMut(u32, f64) -> Result<()> + Send>,
        ) -> Result<Self> {
            Self::start_selected(pid,started,output,name,seconds,mode,close_game,None)
        }
        pub fn start_selected(
            pid:u32,started:f64,output:&Path,name:&str,seconds:u64,mode:&str,
            mut close_game:Box<dyn FnMut(u32,f64)->Result<()>+Send>,
            selection:Option<&crate::native_bridge_assets::Selection>,
        ) -> Result<Self> {
            let resident_runtime = mode == "paid-assignment-runtime";
            let mode = match mode {
                "can-ever" => BridgeMode::CanEver,
                "property" => BridgeMode::Property,
                "property-scoped" => BridgeMode::PropertyScoped,
                "can-ever-calibration" => BridgeMode::CanEverCalibration,
                "property-admission-scoped" => BridgeMode::PropertyAdmissionScoped,
                "joined-admission" => BridgeMode::JoinedAdmission,
                "production-admission" | "paid-assignment-runtime" => {
                    BridgeMode::ProductionAdmission
                }
                "promotion-call" => BridgeMode::PromotionCall,
                "assign-calibration" => BridgeMode::AssignCalibration,
                _ => return Err(Error::scenario("unknown native bridge mode")),
            };
            if !(1..=u64::from(civ6_native_bridge_rs::MAX_ACTIVE_SECONDS)).contains(&seconds) {
                return Err(Error::scenario(
                    "native bridge duration must be 1..300 seconds",
                ));
            }
            let (created, target) = pinned_created(pid, started)?;
            let runner = std::env::current_exe()?;
            let executable =
                if let Some(selection)=selection {selection.directory.join("civ6-native-bridge.exe")}
                else if let Some(directory) = std::env::var_os("CIV6_NATIVE_BRIDGE_DIRECTORY") {
                    let directory = PathBuf::from(directory);
                    if !directory.is_absolute() {
                        return Err(Error::input("native bridge directory must be absolute"));
                    }
                    directory.join("civ6-native-bridge.exe")
                } else {
                    runner.with_file_name("civ6-native-bridge.exe")
                };
            let dll = executable.with_file_name("civ6_native_bridge_rs.dll");
            if !executable.is_file() || !dll.is_file() {
                return Err(Error::input(
                    "native bridge controller or DLL missing beside runner",
                ));
            }
            let artifact = output.join(format!("{name}.jsonl"));
            use std::os::windows::fs::OpenOptionsExt;
            let controller_pin = std::fs::OpenOptions::new()
                .read(true)
                .share_mode(1)
                .open(&executable)?;
            let dll_pin = std::fs::OpenOptions::new()
                .read(true)
                .share_mode(1)
                .open(&dll)?;
            let controller_sha = format!("{:x}", Sha256::digest(std::fs::read(&executable)?));
            let dll_sha = format!("{:x}", Sha256::digest(std::fs::read(&dll)?));
            if let Some(selection)=selection {
                selection.verify(&controller_sha,&dll_sha)?;
            } else if std::env::var_os("CIV6_NATIVE_BRIDGE_DIRECTORY").is_some() {
                for (key, actual) in [
                    ("CIV6_NATIVE_BRIDGE_CONTROLLER_SHA256", &controller_sha),
                    ("CIV6_NATIVE_BRIDGE_DLL_SHA256", &dll_sha),
                ] {
                    verify_override_hash(std::env::var(key).ok().as_deref(), actual)?;
                }
            }
            // The shared release folder can contain a controller from an older
            // branch. Reject its parser before sending PID or installing a DLL.
            query_capabilities(&executable)?;
            if artifact.exists() {
                return Err(Error::scenario("native bridge artifact already exists"));
            }
            let mut manifest = std::fs::File::create_new(artifact.with_extension("assets.json"))?;
            serde_json::to_writer_pretty(
                &mut manifest,
                &serde_json::json!({"runner":runner,
                "controller":executable,"controller_sha256":controller_sha,"dll":dll,"dll_sha256":dll_sha,
                "process_pid":pid,"process_created_filetime":created,"product_acceptance":false}),
            )?;
            manifest.flush()?;
            let mut child = Command::new(executable)
                .args([
                    "--pid",
                    &pid.to_string(),
                    "--created-filetime",
                    &created.to_string(),
                    "--duration",
                    &seconds.to_string(),
                    "--out",
                    &artifact.to_string_lossy(),
                    "--mode",
                    match mode {
                        BridgeMode::CanEver => "can-ever",
                        BridgeMode::Property => "property",
                        BridgeMode::PropertyScoped => "property-scoped",
                        BridgeMode::CanEverCalibration => "can-ever-calibration",
                        BridgeMode::PropertyAdmissionScoped => "property-admission-scoped",
                        BridgeMode::JoinedAdmission => "joined-admission",
                        BridgeMode::ProductionAdmission if resident_runtime => {
                            "paid-assignment-runtime"
                        }
                        BridgeMode::ProductionAdmission => "production-admission",
                        BridgeMode::PromotionCall => "promotion-call",
                        BridgeMode::AssignCalibration => "assign-calibration",
                    },
                ])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .env(
                    "CIV6_AUTOTEST_CONTROLLER_PID",
                    std::process::id().to_string(),
                )
                .env("CIV6_AUTOTEST_RUNNER_PATH", &runner)
                .creation_flags(CREATE_NO_WINDOW)
                .spawn()?;
            let input = child.stdin.take();
            let Some(stdout) = child.stdout.take() else {
                let close = close_game(pid, started);
                if unsafe { WaitForSingleObject(target.0, 10_000) } == WAIT_OBJECT_0 {
                    let _ = child.kill();
                    let _ = child.wait();
                }
                return Err(Error::input(format!(
                    "bridge stdout unavailable; pinned game close={close:?}; exit_confirmed={}",
                    unsafe { WaitForSingleObject(target.0, 0) } == WAIT_OBJECT_0,
                )));
            };
            if input.is_none() {
                let close = close_game(pid, started);
                if unsafe { WaitForSingleObject(target.0, 10_000) } == WAIT_OBJECT_0 {
                    let _ = child.kill();
                    let _ = child.wait();
                }
                return Err(Error::input(format!(
                    "bridge stop pipe unavailable; pinned game close={close:?}; exit_confirmed={}",
                    unsafe { WaitForSingleObject(target.0, 0) } == WAIT_OBJECT_0,
                )));
            }
            let (sender, lines) = mpsc::channel();
            std::thread::spawn(move || {
                for line in BufReader::new(stdout).lines() {
                    let Ok(line) = line else { break };
                    if sender.send(line).is_err() {
                        break;
                    }
                }
            });
            let mut session = Self {
                child,
                input,
                lines,
                artifact,
                pid,
                created,
                started,
                target,
                close_game,
                finished: false,
                mode,
                assignment_subject: None,
                resident_runtime,
                ai_diagnostic: Default::default(),
                reader_requested:false,reader_stop:None,
                _binary_pins: (controller_pin, dll_pin),
            };
            match session.lines.recv_timeout(Duration::from_secs(30)) {
                Ok(line) if line.starts_with(&format!("READY pid={pid} ")) => Ok(session),
                _ => {
                    let close = session.close_target();
                    if unsafe { WaitForSingleObject(session.target.0, 0) } == WAIT_OBJECT_0 {
                        let _ = session.child.kill();
                        let _ = session.child.wait();
                        session.finished = true;
                    }
                    let mut diagnostic = String::new();
                    if session.finished {
                        if let Some(stderr) = session.child.stderr.take() {
                            let _ = stderr.take(16_384).read_to_string(&mut diagnostic);
                        }
                    }
                    Err(Error::input(format!(
                        "native bridge did not prove bounded readiness; pinned game close={close:?}; controller diagnostic={}",
                        diagnostic.trim()
                    )))
                }
            }
        }

        fn close_target(&mut self) -> Result<()> {
            if unsafe { WaitForSingleObject(self.target.0, 0) } == WAIT_OBJECT_0 {
                return Ok(());
            }
            let callback = (self.close_game)(self.pid, self.started);
            let exited = unsafe { WaitForSingleObject(self.target.0, 10_000) } == WAIT_OBJECT_0;
            if !exited {
                return Err(Error::input(format!(
                    "pinned game closure incomplete: callback={callback:?}; target still alive"
                )));
            }
            callback
        }

        fn stop_inner(&mut self) -> Result<PathBuf> {
            if let Some(mut input) = self.input.take() {
                let sent = input.write_all(b"stop\n").and_then(|_| input.flush());
                // A resident observer may have completed at its deadline.
                // EOF also stops this observer; its verified terminal receipt
                // and the child's exit still decide whether stop succeeded.
                if !self.resident_runtime {
                    sent?;
                }
            }
            let waited =
                unsafe { WaitForSingleObject(self.child.as_raw_handle() as HANDLE, 12_000) };
            if waited != WAIT_OBJECT_0 {
                return Err(Error::input("native bridge stop timed out"));
            }
            let status = self.child.wait()?;
            let mut stderr = String::new();
            if let Some(mut stream) = self.child.stderr.take() {
                let _ = stream.read_to_string(&mut stderr);
            }
            if !status.success() {
                return Err(Error::input(format!(
                    "native bridge failed: {}",
                    stderr.trim()
                )));
            }
            if self.resident_runtime {
                validate_runtime_receipt(
                    &std::fs::read_to_string(&self.artifact)?,
                    self.pid,
                    self.created,
                )?;
                // Observation ended; authority and memory still belong to the
                // exact process. Do not report restored entries or close it.
                if unsafe { WaitForSingleObject(self.target.0, 0) } != WAIT_TIMEOUT {
                    return Err(Error::input(
                        "resident runtime game exited before observation stop",
                    ));
                }
            } else {
                terminal_artifact_for_subject(
                    &self.artifact,
                    self.pid,
                    self.created,
                    self.mode,
                    self.assignment_subject,
                )?;
            }
            Ok(self.artifact.clone())
        }

        pub fn select_assignment_subject(
            &mut self,
            observed_pid: u32,
            observed_started: f64,
            subject: AssignmentSubject,
        ) -> Result<()> {
            if self.mode != BridgeMode::AssignCalibration
                || self.assignment_subject.is_some()
                || subject.owner >= 64
                || subject.governor >= 256
                || self.finished
            {
                return Err(Error::input(
                    "assignment subject cannot be selected for this session",
                ));
            }
            let (created, _) = pinned_created(observed_pid, observed_started)?;
            if observed_pid != self.pid
                || created != self.created
                || unsafe { WaitForSingleObject(self.target.0, 0) } == WAIT_OBJECT_0
            {
                return Err(Error::input(
                    "assignment subject belongs to another game process",
                ));
            }
            let file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(self.artifact.with_extension("subject.json"))?;
            serde_json::to_writer(
                file,
                &serde_json::json!({
                    "owner":subject.owner,"governor":subject.governor,
                    "pid":self.pid,"created_filetime":self.created,"payment_authority":false
                }),
            )?;
            self.assignment_subject = Some(subject);
            Ok(())
        }

        pub fn stop(&mut self) -> Result<PathBuf> {
            if self.finished {
                return Err(Error::scenario("native bridge already stopped"));
            }
            let outcome = self.stop_inner();
            if let Err(original) = outcome {
                let closure = self.close_target();
                let cleanup = finish_failed_cleanup(closure, &self.target, &mut self.child);
                if unsafe { WaitForSingleObject(self.target.0, 0) } == WAIT_OBJECT_0
                    && self.child.try_wait()?.is_some()
                {
                    self.finished = true;
                }
                return Err(Error::input(format!(
                    "native bridge incomplete: {original}; cleanup={cleanup:?}"
                )));
            }
            self.finished = true;
            outcome
        }
    }

    impl Drop for Session {
        fn drop(&mut self) {
            if !self.finished {
                let _ = self.stop();
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use serde_json::json;

        #[test]
        fn capability_response_never_waits_for_an_open_writer_to_close() {
            use std::os::windows::io::FromRawHandle;
            let mut read = std::ptr::null_mut();
            let mut write = std::ptr::null_mut();
            assert_ne!(
                unsafe {
                    windows_sys::Win32::System::Pipes::CreatePipe(
                        &mut read,
                        &mut write,
                        std::ptr::null(),
                        0,
                    )
                },
                0
            );
            let mut reader = unsafe { std::fs::File::from_raw_handle(read.cast()) };
            let mut writer = unsafe { std::fs::File::from_raw_handle(write.cast()) };
            let started = std::time::Instant::now();
            assert!(read_capability_response(&mut reader).unwrap().is_empty());
            let expected =
                serde_json::to_vec(&civ6_native_bridge_rs::controller_capabilities()).unwrap();
            writer.write_all(&expected).unwrap();
            assert_eq!(read_capability_response(&mut reader).unwrap(), expected);
            assert!(read_capability_response(&mut reader).unwrap().is_empty());
            assert!(started.elapsed() < Duration::from_secs(1));
            // The same writer is still live: EOF would block, while our read
            // already returned and left it usable for a later observation.
            writer.write_all(b"later").unwrap();
            assert_eq!(read_capability_response(&mut reader).unwrap(), b"later");
        }

        #[test]
        fn stopped_capability_query_reaps_its_child_and_preserves_peer() {
            let mut child = sleeping_child();
            let mut peer = sleeping_child();
            let started = std::time::Instant::now();
            assert!(wait_capability_child(&mut child, Duration::from_millis(50)).is_err());
            assert!(started.elapsed() < Duration::from_secs(3));
            assert!(child.try_wait().unwrap().is_some());
            assert!(peer.try_wait().unwrap().is_none());
            peer.kill().unwrap();
            peer.wait().unwrap();
        }

        fn sleeping_child() -> Child {
            Command::new("powershell.exe")
                .args([
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    "Start-Sleep -Seconds 20",
                ])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .creation_flags(CREATE_NO_WINDOW)
                .spawn()
                .unwrap()
        }

        fn test_target(child: &Child) -> PinnedTarget {
            let handle = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, child.id()) };
            assert!(!handle.is_null());
            PinnedTarget(handle)
        }

        #[test]
        fn expired_runtime_observer_stop_accepts_receipt_and_keeps_target_alive() {
            let mut target = sleeping_child();
            let pinned = test_target(&target);
            let mut controller = Command::new("powershell.exe")
                .args(["-NoProfile", "-NonInteractive", "-Command", "exit 0"])
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .creation_flags(CREATE_NO_WINDOW)
                .spawn()
                .unwrap();
            let input = controller.stdin.take();
            controller.wait().unwrap();
            let dir = tempfile::tempdir().unwrap();
            let artifact = dir.path().join("runtime.jsonl");
            let rows = [
                json!({"kind":"identity","mode":"paid-assignment-runtime","pid":7,"created_filetime":123,
                    "gamecore_sha256":civ6_native_bridge_rs::PINNED_GAMECORE_SHA256,"bridge_sha256":"a".repeat(64),
                    "observation_limit_seconds":1,"runtime_lifetime":"process_exit",
                    "reload_hook_observed":false,"official_ai_payment_observed":false}),
                json!({"kind":"runtime_install","status":0,"three_entries_published":true,
                    "trace_address":4096,"trace_owner":"game_process","bounded_uninstall_allowed":false}),
                json!({"kind":"observation_stop","status":0,"runtime_retained":true,
                    "three_entries_published":true,"trace_retained":true}),
            ];
            std::fs::write(
                &artifact,
                rows.iter()
                    .map(Value::to_string)
                    .collect::<Vec<_>>()
                    .join("\n"),
            )
            .unwrap();
            let (_, lines) = mpsc::channel();
            let mut session = Session {
                child: controller,
                input,
                lines,
                artifact: artifact.clone(),
                pid: 7,
                created: 123,
                started: 0.0,
                target: pinned,
                close_game: Box::new(|_, _| Err(Error::input("must not close fixture target"))),
                finished: false,
                mode: BridgeMode::ProductionAdmission,
                assignment_subject: None,
                resident_runtime: true,
                ai_diagnostic: Default::default(),
                reader_requested:false,reader_stop:None,
                _binary_pins: (
                    std::fs::File::open(&artifact).unwrap(),
                    std::fs::File::open(&artifact).unwrap(),
                ),
            };
            let outcome = session.stop_inner();
            session.finished = true;
            let alive = target.try_wait().unwrap().is_none();
            target.kill().unwrap();
            target.wait().unwrap();
            assert!(alive);
            assert_eq!(outcome.unwrap(), artifact);
        }

        #[test]
        fn failed_stop_closes_only_owned_target_before_controller_and_keeps_peer() {
            let mut target = sleeping_child();
            let mut controller = sleeping_child();
            let mut peer = sleeping_child();
            let pinned = test_target(&target);
            let close = target
                .kill()
                .and_then(|_| target.wait().map(|_| ()))
                .map_err(Error::from);
            finish_failed_cleanup(close, &pinned, &mut controller).unwrap();
            assert!(target.try_wait().unwrap().is_some());
            assert!(controller.try_wait().unwrap().is_some());
            assert!(peer.try_wait().unwrap().is_none());
            peer.kill().unwrap();
            peer.wait().unwrap();
        }

        #[test]
        fn callback_refusal_preserves_live_target_and_controller() {
            let mut target = sleeping_child();
            let mut controller = sleeping_child();
            let mut peer = sleeping_child();
            let pinned = test_target(&target);
            assert!(finish_failed_cleanup(
                Err(Error::input("fixture close refused")),
                &pinned,
                &mut controller,
            )
            .is_err());
            assert!(target.try_wait().unwrap().is_none());
            assert!(controller.try_wait().unwrap().is_none());
            assert!(peer.try_wait().unwrap().is_none());
            target.kill().unwrap();
            target.wait().unwrap();
            controller.kill().unwrap();
            controller.wait().unwrap();
            peer.kill().unwrap();
            peer.wait().unwrap();
        }

        #[test]
        fn terminal_proof_requires_one_restoration_and_one_real_entry() {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("trace.jsonl");
            let rows = [
                json!({"kind":"identity","pid":7,"created_filetime":123,
                    "mode":"can-ever","entry_rva":civ6_native_bridge_rs::PINNED_CAN_EVER_RVA,
                    "gamecore_sha256":civ6_native_bridge_rs::PINNED_GAMECORE_SHA256}),
                json!({"kind":"install","status":0}),
                json!({"kind":"uninstall","status":0,"entry_restored":true,
                    "prefix_retained_until_exit":false}),
                json!({"kind":"entry","sequence":1}),
                json!({"kind":"stop","result":"ok","rows":1,
                    "dropped":0,"observed_calls":1,"in_flight":0,
                    "snapshot_stable":true,"classification":"can_ever_entry",
                    "prefix_retained_until_exit":false}),
            ];
            let write = |rows: &[Value]| {
                std::fs::write(
                    &path,
                    rows.iter()
                        .map(Value::to_string)
                        .collect::<Vec<_>>()
                        .join("\n"),
                )
                .unwrap();
            };
            write(&rows);
            assert_eq!(
                terminal_artifact(&path, 7, 123, BridgeMode::CanEver).unwrap(),
                1
            );
            for index in [0, 1, 2, 3, 4] {
                let mut missing = rows.to_vec();
                missing.remove(index);
                write(&missing);
                assert!(terminal_artifact(&path, 7, 123, BridgeMode::CanEver).is_err());
            }
            let mut wrong = rows.to_vec();
            wrong[2]["status"] = json!(6);
            write(&wrong);
            assert!(terminal_artifact(&path, 7, 123, BridgeMode::CanEver).is_err());
            for (field, value) in [
                ("dropped", json!(1)),
                ("in_flight", json!(1)),
                ("snapshot_stable", json!(false)),
            ] {
                let mut incomplete = rows.to_vec();
                incomplete[4][field] = value;
                write(&incomplete);
                assert!(
                    terminal_artifact(&path, 7, 123, BridgeMode::CanEver).is_err(),
                    "{field}"
                );
            }
        }

        #[test]
        fn promotion_artifact_rejects_other_mode_header_missing_rows_and_overflow() {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("promotion.jsonl");
            let rows = [
                json!({"kind":"identity","pid":7,"created_filetime":123,
                    "mode":"promotion-call","entry_rva":civ6_native_bridge_rs::PINNED_PROMOTION_CALL_RVA,
                    "original_callee_rva":civ6_native_bridge_rs::PINNED_PROMOTION_CALLEE_RVA,
                    "original_return_site_rva":civ6_native_bridge_rs::PINNED_PROMOTION_CALL_RVA+5,
                    "observation_scope":"one_official_promote_call_entry_only",
                    "call_window":"[e8, 9c, ef, fc, ff, 84, c0, 0f, 84, 75, 01, 00, 00, 44, 8b, c7]",
                    "gamecore_sha256":civ6_native_bridge_rs::PINNED_GAMECORE_SHA256}),
                json!({"kind":"install","status":0}),
                json!({"kind":"uninstall","status":0,"entry_restored":true,
                    "prefix_retained_until_exit":false}),
                json!({"kind":"promotion_call_entry","sequence":1,"thread_id":55,
                    "raw_rcx":"0x0000000000000001","raw_rdx":"0x0000000000000002",
                    "raw_r8":"0x0000000000000003","raw_r9":"0x0000000000000004",
                    "return_site_rva":civ6_native_bridge_rs::PINNED_PROMOTION_CALL_RVA+5,
                    "entry_only":true}),
                json!({"kind":"stop","result":"ok","rows":1,"dropped":0,
                    "observed_calls":1,"in_flight":0,"snapshot_stable":true,
                    "quiescence_proven":false,"counter_scope":"snapshot_observation_not_complete_call_history",
                    "classification":"promotion_call_entry_only","prefix_retained_until_exit":false}),
            ];
            let write = |rows: &[Value]| {
                std::fs::write(
                    &path,
                    rows.iter()
                        .map(Value::to_string)
                        .collect::<Vec<_>>()
                        .join("\n"),
                )
                .unwrap()
            };
            write(&rows);
            assert_eq!(
                terminal_artifact(&path, 7, 123, BridgeMode::PromotionCall).unwrap(),
                1
            );
            assert!(terminal_artifact(&path, 7, 123, BridgeMode::CanEver).is_err());
            for (index, field, value) in [
                (0, "mode", json!("can-ever")),
                (0, "call_window", json!("wrong")),
                (3, "return_site_rva", json!(0)),
                (3, "entry_only", json!(false)),
                (4, "dropped", json!(1)),
                (4, "in_flight", json!(1)),
                (4, "quiescence_proven", json!(true)),
                (4, "counter_scope", json!("complete_call_history")),
                (4, "classification", json!("can_ever_entry")),
            ] {
                let mut bad = rows.to_vec();
                bad[index][field] = value;
                write(&bad);
                assert!(
                    terminal_artifact(&path, 7, 123, BridgeMode::PromotionCall).is_err(),
                    "{index}:{field}"
                );
            }
            let mut partial = rows.to_vec();
            for field in ["quiescence_proven", "counter_scope"] {
                let mut missing = rows.to_vec();
                missing[4].as_object_mut().unwrap().remove(field);
                write(&missing);
                assert!(terminal_artifact(&path, 7, 123, BridgeMode::PromotionCall).is_err());
            }
            partial.remove(3);
            write(&partial);
            assert!(terminal_artifact(&path, 7, 123, BridgeMode::PromotionCall).is_err());
        }

        #[test]
        fn property_terminal_classifies_raw_rows_and_keeps_partial_evidence() {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("property.jsonl");
            let rows = [
                json!({"kind":"identity","pid":7,"created_filetime":123,
                    "mode":"property","entry_rva":civ6_native_bridge_rs::PINNED_PROPERTY_GET_RVA,
                    "prefix_bytes":"[cc, cc, cc, cc, cc]",
                    "gamecore_sha256":civ6_native_bridge_rs::PINNED_GAMECORE_SHA256}),
                json!({"kind":"install","status":0}),
                json!({"kind":"uninstall","status":0,"entry_restored":true,
                    "prefix_retained_until_exit":true}),
                json!({"kind":"property_get","sequence":1,"property":"0x0000000000001000",
                    "key_id":853,"returned":"0x0000000000000000","thread_id":4,
                    "read_state":0,"tag":null,"payload8":null,"flags14":null}),
                json!({"kind":"property_get","sequence":2,"property":"0x0000000000001000",
                    "key_id":853,"returned":"0x0000000000002000","thread_id":4,
                    "read_state":1,"tag":2,"payload8":"0x00000000000d0517","flags14":0}),
                json!({"kind":"stop","result":"raw_collected","rows":2,"dropped":0,
                    "observed_calls":2,"in_flight":0,"snapshot_stable":true,
                    "quiescence_proven":false,"classification":"raw_collection",
                    "prefix_retained_until_exit":true}),
            ];
            let write = |rows: &[Value]| {
                std::fs::write(
                    &path,
                    rows.iter()
                        .map(Value::to_string)
                        .collect::<Vec<_>>()
                        .join("\n"),
                )
                .unwrap()
            };
            write(&rows);
            assert_eq!(
                terminal_artifact(&path, 7, 123, BridgeMode::Property).unwrap(),
                2
            );
            let mut one_unmapped = rows.to_vec();
            one_unmapped.remove(4);
            one_unmapped[4]["rows"] = json!(1);
            one_unmapped[4]["observed_calls"] = json!(1);
            write(&one_unmapped);
            assert_eq!(
                terminal_artifact(&path, 7, 123, BridgeMode::Property).unwrap(),
                1
            );
            let mut partial = rows.to_vec();
            partial[5]["result"] = json!("partial_raw");
            partial[5]["dropped"] = json!(3);
            partial[5]["observed_calls"] = json!(5);
            write(&partial);
            assert!(terminal_artifact(&path, 7, 123, BridgeMode::Property)
                .unwrap_err()
                .to_string()
                .contains("3 dropped"));
            for index in 0..rows.len() {
                let mut missing = rows.to_vec();
                missing.remove(index);
                write(&missing);
                assert!(
                    terminal_artifact(&path, 7, 123, BridgeMode::Property).is_err(),
                    "{index}"
                );
            }
            for (field, value) in [
                ("returned", json!("0x0000000000000000")),
                ("tag", json!(null)),
                ("read_state", json!(2)),
            ] {
                let mut bad = rows.to_vec();
                bad[4][field] = value;
                write(&bad);
                assert!(
                    terminal_artifact(&path, 7, 123, BridgeMode::Property).is_err(),
                    "{field}"
                );
            }
        }

        #[test]
        fn scoped_numeric_artifact_rejects_other_keys_and_inconsistent_filter_counts() {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("scoped.jsonl");
            let rows = [
                json!({"kind":"identity","pid":7,"created_filetime":123,
                    "mode":"property-scoped","entry_rva":civ6_native_bridge_rs::PINNED_PROPERTY_GET_RVA,
                    "scope_key_ids":[853271,853272],"prefix_bytes":"[cc, cc, cc, cc, cc]",
                    "gamecore_sha256":civ6_native_bridge_rs::PINNED_GAMECORE_SHA256}),
                json!({"kind":"install","status":0}),
                json!({"kind":"uninstall","status":0,"entry_restored":true,
                    "prefix_retained_until_exit":true}),
                json!({"kind":"property_get","sequence":1,"property":"0x0000000000001000",
                    "key_id":853271,"returned":"0x0000000000000000","thread_id":4,
                    "read_state":0,"tag":null,"payload8":null,"flags14":null}),
                json!({"kind":"property_get","sequence":2,"property":"0x0000000000001000",
                    "key_id":853272,"returned":"0x0000000000002000","thread_id":4,
                    "read_state":1,"tag":2,"payload8":"0x00000000000d0517","flags14":0}),
                json!({"kind":"stop","result":"raw_collected","rows":2,"dropped":0,
                    "observed_calls":2,"filtered_calls":120000,"getter_calls_seen":120002,
                    "in_flight":0,"snapshot_stable":true,"quiescence_proven":false,
                    "classification":"raw_collection","prefix_retained_until_exit":true}),
            ];
            let write = |rows: &[Value]| {
                std::fs::write(
                    &path,
                    rows.iter()
                        .map(Value::to_string)
                        .collect::<Vec<_>>()
                        .join("\n"),
                )
                .unwrap()
            };
            write(&rows);
            assert_eq!(
                terminal_artifact(&path, 7, 123, BridgeMode::PropertyScoped).unwrap(),
                2
            );
            for (index, field, value) in [
                (0, "scope_key_ids", json!([853271, 853270])),
                (3, "key_id", json!(583956078)),
                (4, "key_id", json!(853271)),
                (5, "filtered_calls", json!(119999)),
                (5, "getter_calls_seen", json!(120001)),
            ] {
                let mut bad = rows.to_vec();
                bad[index][field] = value;
                write(&bad);
                assert!(
                    terminal_artifact(&path, 7, 123, BridgeMode::PropertyScoped).is_err(),
                    "{field}"
                );
            }
        }

        #[test]
        fn calibration_requires_both_actual_owners_original_false_and_current_receiver() {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("calibration.jsonl");
            let rows = [
                json!({"kind":"identity","pid":7,"created_filetime":123,
                    "mode":"can-ever-calibration","entry_rva":civ6_native_bridge_rs::PINNED_CAN_EVER_RVA,
                    "gamecore_sha256":civ6_native_bridge_rs::PINNED_GAMECORE_SHA256}),
                json!({"kind":"install","status":0}),
                json!({"kind":"uninstall","status":0,"entry_restored":true,
                    "prefix_retained_until_exit":false}),
                json!({"kind":"calibration","sequence":1,"governor_index":4,
                    "thread_id":11,"owner_read_state":1,"governors_this":"0x0000000000001000",
                    "return_site_rva":0xba523,"player_address":"0x0000000000002000",
                    "player_id":0,"property_receiver":"0x0000000000002528","original_al":0}),
                json!({"kind":"calibration","sequence":2,"governor_index":4,
                    "thread_id":11,"owner_read_state":1,"governors_this":"0x0000000000003000",
                    "return_site_rva":0xba523,"player_address":"0x0000000000004000",
                    "player_id":1,"property_receiver":"0x0000000000004528","original_al":0}),
                json!({"kind":"stop","result":"ok","rows":2,"dropped":0,
                    "observed_calls":2,"in_flight":0,"snapshot_stable":true,
                    "classification":"can_ever_calibration","prefix_retained_until_exit":false}),
            ];
            let write = |rows: &[Value]| {
                std::fs::write(
                    &path,
                    rows.iter()
                        .map(Value::to_string)
                        .collect::<Vec<_>>()
                        .join("\n"),
                )
                .unwrap()
            };
            write(&rows);
            assert_eq!(
                terminal_artifact(&path, 7, 123, BridgeMode::CanEverCalibration).unwrap(),
                2
            );
            for (index, field, value) in [
                (4, "player_id", json!(0)),
                (4, "property_receiver", json!("0x0000000000004529")),
                (4, "original_al", json!(1)),
                (3, "return_site_rva", json!(0xba524)),
            ] {
                let mut bad = rows.to_vec();
                bad[index][field] = value;
                write(&bad);
                assert!(
                    terminal_artifact(&path, 7, 123, BridgeMode::CanEverCalibration).is_err(),
                    "{field}"
                );
            }
        }

        #[test]
        fn assignment_terminal_requires_matching_ui_update_pointers_and_unchanged_original() {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("assign.jsonl");
            let call = json!({"kind":"assign_calibration","sequence":1,"thread_id":11,"read_flags":7,
                "governors_this":"0x0000000000001000","governor_pointer":"0x0000000000003000",
                "city_pointer":"0x0000000000004000","raw_r9":"0x0000000000000000",
                "player_address":"0x0000000000002000","player_id":0,"governor_index":4,
                "city_a8":19,"city_c0":17,"city_c4":29,"city_d8":0,"return_site_rva":0xba3e2,
                "original_rax":"0xaabbccdd00000001","original_al":1,"effective_al":1,
                "promotion_snapshot_status":0,"promotion_owner":0,"promotion_index":4,
                "promotion_count":4096,"owned_promotion_indices":[0,63,64,4095],"payment_authority":false});
            let mut update = call.clone();
            update["sequence"] = json!(2);
            update["return_site_rva"] = json!(0x2dd375);
            let rows = vec![
                json!({"kind":"identity","pid":7,"created_filetime":123,"mode":"assign-calibration",
                    "entry_rva":civ6_native_bridge_rs::PINNED_CAN_ASSIGN_RVA,
                    "gamecore_sha256":civ6_native_bridge_rs::PINNED_GAMECORE_SHA256}),
                json!({"kind":"install","status":0}),
                json!({"kind":"uninstall","status":0,"entry_restored":true,"prefix_retained_until_exit":false}),
                call,
                update,
                json!({"kind":"stop","result":"ok","rows":2,"observed_calls":2,"dropped":0,
                    "in_flight":0,"snapshot_stable":true,"classification":"assign_calibration",
                    "quiescence_proven":false,"counter_scope":"snapshot_observation_not_complete_call_history",
                    "prefix_retained_until_exit":false}),
            ];
            let write = |r: &[Value]| {
                std::fs::write(
                    &path,
                    r.iter()
                        .map(Value::to_string)
                        .collect::<Vec<_>>()
                        .join("\n"),
                )
                .unwrap()
            };
            write(&rows);
            assert_eq!(
                terminal_artifact(&path, 7, 123, BridgeMode::AssignCalibration).unwrap(),
                2
            );
            // A normal person and another owner require their own matching
            // UI/update pair; they cannot certify the old owner0/Index4 probe.
            for subject in [
                AssignmentSubject {
                    owner: 0,
                    governor: 9,
                },
                AssignmentSubject {
                    owner: 3,
                    governor: 12,
                },
            ] {
                let mut normal = rows.clone();
                for row in &mut normal[3..5] {
                    row["player_id"] = json!(subject.owner);
                    row["city_d8"] = json!(subject.owner);
                    row["governor_index"] = json!(subject.governor);
                    row["promotion_owner"] = json!(subject.owner);
                    row["promotion_index"] = json!(subject.governor);
                }
                write(&normal);
                assert!(terminal_artifact(&path, 7, 123, BridgeMode::AssignCalibration).is_err());
                assert_eq!(
                    terminal_artifact_for_subject(
                        &path,
                        7,
                        123,
                        BridgeMode::AssignCalibration,
                        Some(subject)
                    )
                    .unwrap(),
                    2
                );
                for field in ["player_id", "city_d8", "governor_index"] {
                    let mut mixed = normal.clone();
                    mixed[4][field] = json!(99);
                    write(&mixed);
                    assert!(
                        terminal_artifact_for_subject(
                            &path,
                            7,
                            123,
                            BridgeMode::AssignCalibration,
                            Some(subject)
                        )
                        .is_err(),
                        "{field}"
                    );
                }
            }
            write(&rows);
            assert!(terminal_artifact_for_subject(
                &path,
                7,
                123,
                BridgeMode::AssignCalibration,
                Some(AssignmentSubject {
                    owner: 64,
                    governor: 4
                })
            )
            .is_err());
            assert!(terminal_artifact_for_subject(
                &path,
                7,
                123,
                BridgeMode::AssignCalibration,
                Some(AssignmentSubject {
                    owner: 0,
                    governor: 256
                })
            )
            .is_err());
            for (index, field, value) in [
                (3, "original_rax", json!("0xaabbccdd00000000")),
                (3, "effective_al", json!(0)),
                (3, "governor_pointer", json!(4)),
                (4, "governor_pointer", json!("0x0000000000003008")),
                (4, "read_flags", json!(3)),
                (4, "city_a8", json!(20)),
                (4, "raw_r9", json!("0x0000000000007777")),
                (4, "return_site_rva", json!(0x2dd370)),
                (5, "dropped", json!(1)),
                (5, "quiescence_proven", json!(true)),
                (5, "counter_scope", json!("complete_call_history")),
                (2, "entry_restored", json!(false)),
                (0, "entry_rva", json!(0x2dddd0)),
                (3, "promotion_snapshot_status", json!(8)),
                (3, "promotion_owner", json!(1)),
                (3, "promotion_index", json!(5)),
                (3, "promotion_count", json!(0)),
                (3, "promotion_count", json!(4097)),
                (3, "owned_promotion_indices", json!([0, 63, 63])),
                (3, "owned_promotion_indices", json!([64, 63])),
                (3, "owned_promotion_indices", json!([4096])),
                (3, "owned_promotion_indices", json!(["1"])),
                (3, "payment_authority", json!(true)),
            ] {
                let mut bad = rows.clone();
                bad[index][field] = value;
                write(&bad);
                assert!(
                    terminal_artifact(&path, 7, 123, BridgeMode::AssignCalibration).is_err(),
                    "{field}"
                );
            }
            for field in ["quiescence_proven", "counter_scope"] {
                let mut bad = rows.clone();
                bad[5].as_object_mut().unwrap().remove(field);
                write(&bad);
                assert!(
                    terminal_artifact(&path, 7, 123, BridgeMode::AssignCalibration).is_err(),
                    "missing {field}"
                );
            }
            for field in [
                "promotion_snapshot_status",
                "promotion_owner",
                "promotion_index",
                "promotion_count",
                "owned_promotion_indices",
                "payment_authority",
            ] {
                let mut bad = rows.clone();
                bad[3].as_object_mut().unwrap().remove(field);
                write(&bad);
                assert!(
                    terminal_artifact(&path, 7, 123, BridgeMode::AssignCalibration).is_err(),
                    "missing {field}"
                );
            }
            let mut unreadable = rows.clone();
            for row in &mut unreadable[3..5] {
                row["promotion_snapshot_status"] = json!(1);
                row["promotion_owner"] = json!(u32::MAX);
                row["promotion_index"] = json!(u32::MAX);
                row["promotion_count"] = json!(0);
                row["owned_promotion_indices"] = json!([]);
            }
            write(&unreadable);
            assert_eq!(
                terminal_artifact(&path, 7, 123, BridgeMode::AssignCalibration).unwrap(),
                2
            );
        }

        #[test]
        fn admission_property_requires_typed_false_true_and_distinct_threads() {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("admission-property.jsonl");
            let rows = [
                json!({"kind":"identity","pid":7,"created_filetime":123,
                    "mode":"property-admission-scoped","entry_rva":civ6_native_bridge_rs::PINNED_PROPERTY_GET_RVA,
                    "scope_key_ids":[853304],"prefix_bytes":"[cc, cc, cc, cc, cc]",
                    "gamecore_sha256":civ6_native_bridge_rs::PINNED_GAMECORE_SHA256}),
                json!({"kind":"install","status":0}),
                json!({"kind":"uninstall","status":0,"entry_restored":true,
                    "prefix_retained_until_exit":true}),
                json!({"kind":"property_get","sequence":1,"property":"0x0000000000002528",
                    "key_id":853304,"returned":"0x0000000000006000","thread_id":11,
                    "read_state":1,"tag":1,"payload8":"0x0000000000000000","flags14":0}),
                json!({"kind":"property_get","sequence":2,"property":"0x0000000000002528",
                    "key_id":853304,"returned":"0x0000000000006010","thread_id":12,
                    "read_state":1,"tag":1,"payload8":"0x0000000000000001","flags14":0}),
                json!({"kind":"stop","result":"raw_collected","rows":2,"dropped":0,
                    "observed_calls":2,"filtered_calls":3,"getter_calls_seen":5,
                    "in_flight":0,"snapshot_stable":true,"quiescence_proven":false,
                    "classification":"raw_collection","prefix_retained_until_exit":true}),
            ];
            let write = |rows: &[Value]| {
                std::fs::write(
                    &path,
                    rows.iter()
                        .map(Value::to_string)
                        .collect::<Vec<_>>()
                        .join("\n"),
                )
                .unwrap()
            };
            write(&rows);
            assert_eq!(
                terminal_artifact(&path, 7, 123, BridgeMode::PropertyAdmissionScoped).unwrap(),
                2
            );
            for (index, field, value) in [
                (4, "tag", json!(2)),
                (4, "payload8", json!("0x0000000000000002")),
                (4, "thread_id", json!(11)),
                (0, "scope_key_ids", json!([853271])),
            ] {
                let mut bad = rows.to_vec();
                bad[index][field] = value;
                write(&bad);
                assert!(
                    terminal_artifact(&path, 7, 123, BridgeMode::PropertyAdmissionScoped).is_err(),
                    "{field}"
                );
            }
            let mut reversed = rows.to_vec();
            reversed[3]["payload8"] = json!("0x0000000000000001");
            reversed[4]["payload8"] = json!("0x0000000000000000");
            write(&reversed);
            assert!(terminal_artifact(&path, 7, 123, BridgeMode::PropertyAdmissionScoped).is_err());
        }

        #[test]
        fn joined_terminal_requires_positive_bound_to_core_bool_and_both_restorations() {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("joined.jsonl");
            let rows = [
                json!({"kind":"identity","pid":7,"created_filetime":123,
                    "mode":"joined-admission","entry_rva":civ6_native_bridge_rs::PINNED_CAN_EVER_RVA,
                    "property_rva":civ6_native_bridge_rs::PINNED_PROPERTY_GET_RVA,
                    "scope_key_ids":[853304],"prefix_bytes":"[cc, cc, cc, cc, cc]",
                    "gamecore_sha256":civ6_native_bridge_rs::PINNED_GAMECORE_SHA256}),
                json!({"kind":"install","status":0}),
                json!({"kind":"uninstall","status":0,"entry_restored":true,
                    "prefix_retained_until_exit":true}),
                json!({"kind":"calibration","sequence":1,"governor_index":4,
                    "thread_id":11,"owner_read_state":1,"governors_this":"0x0000000000001000",
                    "return_site_rva":0xba523,"player_address":"0x0000000000002000",
                    "player_id":0,"property_receiver":"0x0000000000002528",
                    "original_al":0,"effective_al":0}),
                json!({"kind":"calibration","sequence":2,"governor_index":4,
                    "thread_id":11,"owner_read_state":1,"governors_this":"0x0000000000001000",
                    "return_site_rva":0xba523,"player_address":"0x0000000000002000",
                    "player_id":0,"property_receiver":"0x0000000000002528",
                    "original_al":0,"effective_al":1}),
                json!({"kind":"property_get","sequence":1,"property":"0x0000000000002528",
                    "key_id":853304,"returned":"0x0000000000006000","thread_id":12,
                    "read_state":1,"tag":1,"payload8":"0x0000000000000000","flags14":0}),
                json!({"kind":"property_get","sequence":2,"property":"0x0000000000002528",
                    "key_id":853304,"returned":"0x0000000000006010","thread_id":12,
                    "read_state":1,"tag":1,"payload8":"0x0000000000000001","flags14":0}),
                json!({"kind":"property_get","sequence":3,"property":"0x0000000000003528",
                    "key_id":853304,"returned":"0x0000000000006020","thread_id":11,
                    "read_state":1,"tag":1,"payload8":"0x0000000000000000","flags14":0}),
                json!({"kind":"stop","result":"ok","rows":5,"dropped":0,
                    "observed_calls":5,"filtered_calls":0,"getter_calls_seen":5,
                    "in_flight":0,"snapshot_stable":true,"classification":"joined_admission",
                    "prefix_retained_until_exit":true}),
            ];
            let write = |rows: &[Value]| {
                std::fs::write(
                    &path,
                    rows.iter()
                        .map(Value::to_string)
                        .collect::<Vec<_>>()
                        .join("\n"),
                )
                .unwrap()
            };
            write(&rows);
            assert_eq!(
                terminal_artifact(&path, 7, 123, BridgeMode::JoinedAdmission).unwrap(),
                5
            );
            let mut bad = rows.to_vec();
            bad[4]["property_receiver"] = json!("0x0000000000004528");
            write(&bad);
            assert!(terminal_artifact(&path, 7, 123, BridgeMode::JoinedAdmission).is_err());
            let mut bad = rows.to_vec();
            bad[4]["effective_al"] = json!(0);
            write(&bad);
            assert!(terminal_artifact(&path, 7, 123, BridgeMode::JoinedAdmission).is_err());
        }

        #[test]
        fn production_terminal_accepts_invalid_raw_value_as_denial_evidence_only() {
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("production.jsonl");
            let rows = [
                json!({"kind":"identity","pid":7,"created_filetime":123,
                    "mode":"production-admission","entry_rva":civ6_native_bridge_rs::PINNED_CAN_EVER_RVA,
                    "property_rva":civ6_native_bridge_rs::PINNED_PROPERTY_GET_RVA,
                    "scope_key_ranges":[[900000,900255],[901000,901255],[903000,903255]],
                    "reset_key_range":[902000,902063],"prefix_bytes":"[cc, cc, cc, cc, cc]",
                    "gamecore_sha256":civ6_native_bridge_rs::PINNED_GAMECORE_SHA256}),
                json!({"kind":"install","status":0}),
                json!({"kind":"uninstall","status":0,"entry_restored":true,
                    "prefix_retained_until_exit":true}),
                json!({"kind":"calibration","sequence":1,"governor_index":77,
                    "thread_id":11,"owner_read_state":1,"governors_this":"0x0000000000001000",
                    "player_address":"0x0000000000002000","player_id":2,
                    "property_receiver":"0x0000000000002528",
                    "original_al":0,"effective_al":0}),
                json!({"kind":"property_get","sequence":1,"property":"0x0000000000002528",
                    "key_id":901077,"returned":"0x0000000000006000","thread_id":12,
                    "read_state":1,"tag":2,"payload8":"0x0000000000000001","flags14":0}),
                json!({"kind":"stop","result":"ok","rows":2,"dropped":0,
                    "observed_calls":2,"filtered_calls":0,"getter_calls_seen":2,
                    "in_flight":0,"snapshot_stable":true,"classification":"production_admission",
                    "prefix_retained_until_exit":true}),
            ];
            let write = |rows: &[Value]| {
                std::fs::write(
                    &path,
                    rows.iter()
                        .map(Value::to_string)
                        .collect::<Vec<_>>()
                        .join("\n"),
                )
                .unwrap();
            };
            write(&rows);
            assert_eq!(
                terminal_artifact(&path, 7, 123, BridgeMode::ProductionAdmission).is_err(),
                true
            );
            let mut verified = rows.to_vec();
            verified[4]["denial_checked"] = json!(true);
            verified[4]["denial_owner"] = json!(2);
            verified[4]["denial_index"] = json!(77);
            write(&verified);
            assert!(
                terminal_artifact(&path, 7, 123, BridgeMode::ProductionAdmission).is_err(),
                "a production artifact without a paired owner reset is incomplete"
            );
            verified.insert(
                4,
                json!({"kind":"property_get","sequence":1,
                "property":"0x0000000000002528","key_id":902002,
                "returned":"0x0000000000006000","thread_id":12,
                "read_state":1,"tag":1,"payload8":"0x0000000000000001","flags14":0}),
            );
            verified[5]["sequence"] = json!(2);
            verified[6]["rows"] = json!(3);
            verified[6]["observed_calls"] = json!(3);
            verified[6]["getter_calls_seen"] = json!(3);
            write(&verified);
            assert_eq!(
                terminal_artifact(&path, 7, 123, BridgeMode::ProductionAdmission).unwrap(),
                3
            );
            for (field, value) in [
                ("key_id", json!(902001)),
                ("property", json!("0x0000000000003000")),
                ("payload8", json!("0x0000000000000000")),
            ] {
                let mut bad = verified.clone();
                bad[4][field] = value;
                write(&bad);
                assert!(
                    terminal_artifact(&path, 7, 123, BridgeMode::ProductionAdmission).is_err(),
                    "reset {field} must identify the calibrated Core owner"
                );
            }
            for (field, value) in [
                ("denial_checked", json!(false)),
                ("denial_index", json!(78)),
                ("denial_owner", json!(64)),
                ("denial_owner", json!(1)),
            ] {
                let mut bad = verified.clone();
                bad[5][field] = value;
                write(&bad);
                assert!(
                    terminal_artifact(&path, 7, 123, BridgeMode::ProductionAdmission).is_err(),
                    "{field}"
                );
            }
            let mut sampled = verified.clone();
            sampled[6]["dropped"] = json!(80);
            sampled[6]["observed_calls"] = json!(83);
            sampled[6]["counter_scope"] = json!("bounded_prefix_samples_not_complete_call_history");
            sampled[6]["calibration_sample_limit"] = json!(1024);
            sampled[6]["property_sample_limit"] = json!(4096);
            write(&sampled);
            assert_eq!(
                terminal_artifact(&path, 7, 123, BridgeMode::ProductionAdmission).unwrap(),
                3
            );
            sampled[6]["counter_scope"] = json!("complete_history");
            write(&sampled);
            assert!(terminal_artifact(&path, 7, 123, BridgeMode::ProductionAdmission).is_err());
            let mut foreign = rows.to_vec();
            foreign[4]["key_id"] = json!(853304);
            write(&foreign);
            assert!(terminal_artifact(&path, 7, 123, BridgeMode::ProductionAdmission).is_err());
            let mut unverified = rows.to_vec();
            unverified[2]["entry_restored"] = json!(false);
            write(&unverified);
            assert!(terminal_artifact(&path, 7, 123, BridgeMode::ProductionAdmission).is_err());
        }
    }
}

#[cfg(windows)]
pub use platform::{close_pinned_game, Session};

#[cfg(not(windows))]
pub fn close_pinned_game(_: u32, _: f64) -> Result<()> {
    Err(Error::input("native bridge requires Windows x64"))
}

#[cfg(not(windows))]
pub struct Session;
#[cfg(not(windows))]
impl Session {
    pub fn start_selected(_:u32,_:f64,_:&Path,_:&str,_:u64,_:&str,_:Box<dyn FnMut(u32,f64)->Result<()>+Send>,_:Option<&crate::native_bridge_assets::Selection>)->Result<Self> {
        Err(Error::input("native bridge requires Windows x64"))
    }
    pub fn ai_diagnostic(&mut self, _: &Value) -> Result<Value> {
        Err(Error::input("AI diagnostics require Windows"))
    }
    pub fn select_assignment_subject(
        &mut self,
        _: u32,
        _: f64,
        _: AssignmentSubject,
    ) -> Result<()> {
        Err(Error::input(
            "native assignment observation requires Windows",
        ))
    }
    pub fn process_identity(&self) -> (u32, u64) {
        (0, 0)
    }
    pub fn retains_process_after_stop(&self) -> bool {
        false
    }
    pub fn process_has_exited(&self) -> bool {
        false
    }
    pub fn verify_current_process(&self, _: u32, _: f64) -> Result<(u32, u64)> {
        Err(Error::input("native bridge requires Windows x64"))
    }
    pub fn start(
        _pid: u32,
        _started: f64,
        _output: &Path,
        _name: &str,
        _seconds: u64,
        _mode: &str,
        _close_game: Box<dyn FnMut(u32, f64) -> Result<()> + Send>,
    ) -> Result<Self> {
        Err(Error::input("native bridge requires Windows x64"))
    }
    pub fn stop(&mut self) -> Result<PathBuf> {
        Err(Error::input("native bridge requires Windows x64"))
    }
}
