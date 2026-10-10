//! Strict, separate log contract for the native-carrier save fixture.
//! The original growth witness keeps its 21-step/49-field contract unchanged.
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_BODY: usize = 1_048_576;
pub const MAX_PARTS: usize = MAX_BODY.div_ceil(96);
pub const MAX_FRAMES: usize = 256;
pub const MAX_RUN_BODY: usize = 16 * MAX_BODY;

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    pub nonce: String,
    pub scene: String,
    pub serial: u64,
    pub generation: u8,
    pub step: u8,
    pub kind: String,
    pub body: String,
}

#[derive(Clone, Debug)]
struct Chunk {
    frame: Frame,
    part: usize,
    total: usize,
    hex: String,
}

fn problem(message: &str) -> Error {
    Error::scenario(format!("native carrier witness: {message}"))
}

pub fn nonce_valid(value: &str) -> bool {
    value.len() == 32 && value.bytes().all(|b| b.is_ascii_hexdigit())
}

fn decimal<T: std::str::FromStr>(value: &str) -> Result<T> {
    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err(problem("nondecimal header"));
    }
    value.parse().map_err(|_| problem("header overflow"))
}

fn chunk(line: &str, nonce: &str) -> Result<Option<Chunk>> {
    let Some(start) = line.find("JAC_NC|") else {
        return Ok(None);
    };
    let f: Vec<_> = line[start..].trim().split('|').collect();
    if f.len() != 10 || f[0] != "JAC_NC" || f[1] != "1" {
        return Err(problem("invalid frame version/fields"));
    }
    if !nonce_valid(f[2]) || f[2] != nonce {
        return Err(problem("foreign or invalid nonce"));
    }
    if f[3].is_empty()
        || f[3].len() > 128
        || !f[3].bytes().all(|b| b.is_ascii_alphanumeric() || b == b'.')
        || f[7].is_empty()
        || f[7].len() > 40
        || !f[7].bytes().all(|b| b.is_ascii_uppercase() || b == b'_')
    {
        return Err(problem("invalid scene/kind"));
    }
    let (part, total) = f[8].split_once('/').ok_or_else(|| problem("part index"))?;
    let parsed = Chunk {
        frame: Frame {
            nonce: f[2].into(),
            scene: f[3].into(),
            serial: decimal(f[4])?,
            generation: decimal(f[5])?,
            step: decimal(f[6])?,
            kind: f[7].into(),
            body: String::new(),
        },
        part: decimal(part)?,
        total: decimal(total)?,
        hex: f[9].into(),
    };
    if parsed.frame.serial == 0
        || parsed.frame.generation > 2
        || parsed.frame.step > 43
        || parsed.total == 0
        || parsed.total > MAX_PARTS
        || parsed.part == 0
        || parsed.part > parsed.total
        || parsed.hex.is_empty()
        || parsed.hex.len() > 192
        || !parsed.hex.len().is_multiple_of(2)
        || !parsed.hex.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return Err(problem("invalid bounded header/payload"));
    }
    Ok(Some(parsed))
}

/// Only a final, incomplete packet may remain while the producer prints it.
/// Missing/interleaved/repeated chunks never become a successful frame.
pub fn parse_available_frames(log: &str, nonce: &str) -> Result<Vec<Frame>> {
    if !nonce_valid(nonce) {
        return Err(problem("invalid expected nonce"));
    }
    let mut frames = Vec::new();
    let mut pending: Option<Chunk> = None;
    let mut payload = Vec::new();
    let mut next = 1;
    let mut consumed = 0usize;
    for line in log.lines() {
        let Some(c) = chunk(line, nonce)? else {
            continue;
        };
        if pending.is_none() {
            if c.part != 1 {
                return Err(problem("packet starts after first chunk"));
            }
            pending = Some(c.clone());
        }
        let p = pending.as_ref().unwrap();
        if p.frame != c.frame || p.total != c.total || c.part != next {
            return Err(problem("interleaved/missing/repeated chunk"));
        }
        for pair in c.hex.as_bytes().chunks_exact(2) {
            let digit = |v: u8| match v {
                b'0'..=b'9' => v - b'0',
                b'a'..=b'f' => v - b'a' + 10,
                b'A'..=b'F' => v - b'A' + 10,
                _ => unreachable!("hex validated"),
            };
            payload.push(digit(pair[0]) * 16 + digit(pair[1]));
        }
        if payload.len() > MAX_BODY {
            return Err(problem("payload exceeds byte budget"));
        }
        if c.part == c.total {
            let mut complete = pending.take().unwrap().frame;
            complete.body = String::from_utf8(std::mem::take(&mut payload))
                .map_err(|_| problem("payload is not UTF-8"))?;
            consumed = consumed
                .checked_add(complete.body.len())
                .ok_or_else(|| problem("run byte overflow"))?;
            if frames.len() >= MAX_FRAMES || consumed > MAX_RUN_BODY {
                return Err(problem("run frame/byte budget exceeded"));
            }
            frames.push(complete);
            next = 1;
        } else {
            next += 1;
        }
    }
    Ok(frames)
}

/// One exact latest packet per scene makes repeated read-only polling harmless.
/// Older packets and changed bodies under a consumed serial are rejected.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Receipts {
    latest: BTreeMap<String, Frame>,
}

impl Receipts {
    pub fn accept(&mut self, frame: &Frame) -> Result<bool> {
        if let Some(old) = self.latest.get(&frame.scene) {
            if frame.serial == old.serial {
                return if frame == old {
                    Ok(false)
                } else {
                    Err(problem("consumed serial changed"))
                };
            }
            if Some(frame.serial) != old.serial.checked_add(1) {
                return Err(problem("serial gap or stale packet"));
            }
        } else if frame.serial != 1 || self.latest.len() >= 32 {
            return Err(problem(
                "new scene must begin at serial one within scene budget",
            ));
        }
        self.latest.insert(frame.scene.clone(), frame.clone());
        Ok(true)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Action {
    Begin { scene: String },
    AcceptDialog { scene: String, serial: u64 },
    Load { generation: u8 },
    Complete,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Payload {
    pub generation: u8,
    pub canonical: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
}

impl Frame {
    pub fn payload(&self) -> Result<Payload> {
        let data: Payload = serde_json::from_str(&self.body)?;
        if data.generation != self.generation || data.canonical.is_empty() {
            return Err(problem("body generation/record mismatch"));
        }
        Ok(data)
    }
}

pub fn save_name(nonce: &str, generation: u8) -> Result<String> {
    if !nonce_valid(nonce) {
        return Err(problem("invalid save nonce"));
    }
    let label = match generation {
        1 => "MidLoan",
        2 => "Returned",
        _ => return Err(problem("invalid save generation")),
    };
    Ok(format!("JAC-NativeCarrier-{label}-{nonce}"))
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Journal {
    pub nonce: String,
    pub owner: civ6_autotest_rs::hotseat_bridge::WindowOwner,
    receipts: Receipts,
    scenes: BTreeMap<String, u8>,
    save_snapshots: BTreeMap<u8, String>,
    save_complete: BTreeSet<u8>,
    pub load_sent: BTreeSet<u8>,
    begin_sent: BTreeSet<String>,
    dialog_sent: BTreeSet<(String, u64)>,
    pub complete: bool,
}

impl Journal {
    pub fn new(nonce: &str, owner: civ6_autotest_rs::hotseat_bridge::WindowOwner) -> Result<Self> {
        if !nonce_valid(nonce) || owner.pid == 0 || owner.hwnd == 0 || !owner.started.is_finite() {
            return Err(problem("invalid run/window ownership"));
        }
        Ok(Self {
            nonce: nonce.into(),
            owner,
            receipts: Receipts::default(),
            scenes: BTreeMap::new(),
            save_snapshots: BTreeMap::new(),
            save_complete: BTreeSet::new(),
            load_sent: BTreeSet::new(),
            begin_sent: BTreeSet::new(),
            dialog_sent: BTreeSet::new(),
            complete: false,
        })
    }

    /// Returns an action only once. Persist the entire journal before issuing it.
    /// A failed or unknown input/load keeps the sent marker and stops the run.
    pub fn consume(&mut self, f: &Frame) -> Result<Option<Action>> {
        if f.nonce != self.nonce || self.complete {
            return Err(problem("foreign frame or event after completion"));
        }
        if !self.receipts.accept(f)? {
            return Ok(None);
        }
        let payload = f.payload()?;
        if f.kind == "FAIL" {
            return Err(problem(&format!(
                "fixture failed: {:?}: {:?}",
                payload.code, payload.error
            )));
        }
        match f.kind.as_str() {
            "BOOT" => {
                if self.scenes.contains_key(&f.scene)
                    || (f.generation > 0 && !self.load_sent.contains(&f.generation))
                {
                    return Err(problem("unissued load or repeated BOOT scene"));
                }
                self.scenes.insert(f.scene.clone(), f.generation);
            }
            "SAVE_RETURNED" => {
                if f.generation == 0
                    || self.save_snapshots.contains_key(&f.generation)
                    || (f.generation == 2
                        && (!self.load_sent.contains(&1) || !self.scenes.values().any(|&g| g == 1)))
                    || payload.name.as_ref() != Some(&save_name(&self.nonce, f.generation)?)
                {
                    return Err(problem("save generation reused"));
                }
                self.save_snapshots
                    .insert(f.generation, payload.canonical.clone());
            }
            "SAVE_COMPLETE" => {
                if self.save_snapshots.get(&f.generation) != Some(&payload.canonical)
                    || payload.name.as_ref() != Some(&save_name(&self.nonce, f.generation)?)
                    || !self.save_complete.insert(f.generation)
                {
                    return Err(problem("save completion lacks matching exact snapshot"));
                }
            }
            "EXIT_REQUESTED" => {
                if !self.save_complete.contains(&f.generation)
                    || !self.load_sent.insert(f.generation)
                {
                    return Err(problem("exit/load before save completion or repeated load"));
                }
                return Ok(Some(Action::Load {
                    generation: f.generation,
                }));
            }
            "LOAD_READY" => {
                if self.scenes.get(&f.scene) != Some(&f.generation)
                    || !self.begin_sent.insert(f.scene.clone())
                {
                    return Err(problem("unadopted or repeated begin"));
                }
                return Ok(Some(Action::Begin {
                    scene: f.scene.clone(),
                }));
            }
            "DIALOG_OPEN" => {
                if self.scenes.get(&f.scene) != Some(&f.generation)
                    || !self.dialog_sent.insert((f.scene.clone(), f.serial))
                {
                    return Err(problem("unadopted dialog"));
                }
                return Ok(Some(Action::AcceptDialog {
                    scene: f.scene.clone(),
                    serial: f.serial,
                }));
            }
            "DONE" => {
                if f.generation != 2
                    || !self.load_sent.contains(&2)
                    || !self.save_complete.contains(&2)
                    || self.scenes.get(&f.scene) != Some(&2)
                    || !self.load_sent.contains(&1)
                    || !self.save_complete.contains(&1)
                    || self.save_snapshots.get(&2) != Some(&payload.canonical)
                {
                    return Err(problem(
                        "final snapshot differs or saved reload is unproved",
                    ));
                }
                self.complete = true;
                return Ok(Some(Action::Complete));
            }
            "STAGING_READY" | "DIALOG_CLOSED" | "RESULT" => {}
            _ => return Err(problem("unknown event kind")),
        }
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn nonce() -> &'static str {
        static VALUE: std::sync::OnceLock<String> = std::sync::OnceLock::new();
        VALUE
            .get_or_init(|| uuid::Uuid::new_v4().simple().to_string())
            .as_str()
    }
    fn packet(serial: u64, part: usize, total: usize, body: &str) -> String {
        let run_id = nonce();
        let hex: String = body.bytes().map(|b| format!("{b:02x}")).collect();
        format!("witness: JAC_NC|1|{run_id}|12.5|{serial}|1|25|RESULT|{part}/{total}|{hex}")
    }

    #[test]
    fn native_steps_above_the_unchanged_growth_limit_and_utf8_are_retained() {
        let run_id = nonce();
        let frames = parse_available_frames(&packet(1, 1, 1, "原本"), nonce()).unwrap();
        assert_eq!(frames[0].step, 25);
        assert_eq!(frames[0].body, "原本");
        let old = format!("JAC_HS|1|{run_id}|12.5|1|21|RESULT|1/1|e58e9fe69cac");
        assert_eq!(
            civ6_autotest_rs::hotseat_bridge::parse_frames(&old, nonce()).unwrap()[0].body,
            "原本"
        );
        assert!(civ6_autotest_rs::hotseat_bridge::parse_frames(
            &old.replace("|21|", "|25|"),
            nonce()
        )
        .is_err());
    }

    #[test]
    fn partial_delivery_is_not_success_and_duplicates_gaps_and_interleaving_fail() {
        let first = packet(1, 1, 2, "source");
        assert!(parse_available_frames(&first, nonce()).unwrap().is_empty());
        let good = format!("{first}\n{}", packet(1, 2, 2, " returned"));
        assert_eq!(
            parse_available_frames(&good, nonce()).unwrap()[0].body,
            "source returned"
        );
        for tail in [
            first.clone(),
            packet(1, 3, 3, "missing"),
            packet(2, 2, 2, "interleaved"),
        ] {
            assert!(parse_available_frames(&format!("{first}\n{tail}"), nonce()).is_err());
        }
    }

    #[test]
    fn foreign_nonce_invalid_utf8_and_unbounded_headers_are_rejected() {
        let good = packet(1, 1, 1, "real");
        let foreign = uuid::Uuid::new_v4().simple().to_string();
        assert_ne!(foreign, nonce());
        for bad in [
            good.replace(nonce(), &foreign),
            good.replace("|1|25|", "|3|25|"),
            good.replace("|25|", "|44|"),
            good.replace("|1/1|7265616c", "|1/1|ff"),
            good.replace("|1/1|", "|1/10924|"),
        ] {
            assert!(parse_available_frames(&bad, nonce()).is_err(), "{bad}");
        }
    }

    #[test]
    fn receipt_exact_repoll_is_harmless_but_replays_and_body_changes_fail() {
        let mut receipts = Receipts::default();
        let first = parse_available_frames(&packet(1, 1, 1, "one"), nonce())
            .unwrap()
            .remove(0);
        assert!(receipts.accept(&first).unwrap());
        assert!(!receipts.accept(&first).unwrap());
        let mut changed = first.clone();
        changed.body = "forged".into();
        assert!(receipts.accept(&changed).is_err());
        let second = parse_available_frames(&packet(2, 1, 1, "two"), nonce())
            .unwrap()
            .remove(0);
        assert!(receipts.accept(&second).unwrap());
        assert!(receipts.accept(&first).is_err());
    }

    #[test]
    fn total_frame_budget_is_enforced_without_serial_arithmetic_panics() {
        let log = (1..=MAX_FRAMES + 1)
            .map(|n| packet(n as u64, 1, 1, "real"))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(parse_available_frames(&log, nonce()).is_err());
        let mut receipts = Receipts::default();
        let mut frame = parse_available_frames(&packet(1, 1, 1, "real"), nonce())
            .unwrap()
            .remove(0);
        receipts.accept(&frame).unwrap();
        frame.serial = u64::MAX;
        assert!(receipts.accept(&frame).is_err());
    }

    fn event(scene: &str, serial: u64, generation: u8, kind: &str, canonical: &str) -> Frame {
        let name = if kind.starts_with("SAVE_") {
            Some(save_name(nonce(), generation).unwrap())
        } else {
            None
        };
        let mut body = serde_json::json!({"generation":generation,"canonical":canonical});
        if let Some(name) = name {
            body["name"] = name.into();
        }
        Frame {
            nonce: nonce().into(),
            scene: scene.into(),
            serial,
            generation,
            step: 0,
            kind: kind.into(),
            body: body.to_string(),
        }
    }

    fn journal() -> Journal {
        Journal::new(
            nonce(),
            civ6_autotest_rs::hotseat_bridge::WindowOwner {
                pid: 1,
                started: 1.,
                hwnd: 1,
            },
        )
        .unwrap()
    }

    #[test]
    fn real_save_notifications_two_issued_loads_and_exact_final_snapshot_are_required() {
        let mut j = journal();
        j.consume(&event("initial", 1, 0, "BOOT", "initial"))
            .unwrap();
        j.consume(&event("initial", 2, 1, "SAVE_RETURNED", "borrowed"))
            .unwrap();
        j.consume(&event("initial", 3, 1, "SAVE_COMPLETE", "borrowed"))
            .unwrap();
        assert_eq!(
            j.consume(&event("initial", 4, 1, "EXIT_REQUESTED", "exit"))
                .unwrap(),
            Some(Action::Load { generation: 1 })
        );
        j.consume(&event("midload", 1, 1, "BOOT", "borrowed"))
            .unwrap();
        let begin = event("midload", 2, 1, "LOAD_READY", "armed");
        assert!(matches!(
            j.consume(&begin).unwrap(),
            Some(Action::Begin { .. })
        ));
        assert_eq!(j.consume(&begin).unwrap(), None);
        j.consume(&event("midload", 3, 2, "SAVE_RETURNED", "returned"))
            .unwrap();
        j.consume(&event("midload", 4, 2, "SAVE_COMPLETE", "returned"))
            .unwrap();
        j.consume(&event("midload", 5, 2, "EXIT_REQUESTED", "exit"))
            .unwrap();
        j.consume(&event("final", 1, 2, "BOOT", "returned"))
            .unwrap();
        assert_eq!(
            j.consume(&event("final", 2, 2, "DONE", "returned"))
                .unwrap(),
            Some(Action::Complete)
        );
    }

    #[test]
    fn early_final_save_unissued_boot_mismatched_completion_and_changed_final_cannot_pass() {
        let mut j = journal();
        assert!(j
            .consume(&event("final", 1, 2, "BOOT", "returned"))
            .is_err());
        let mut j = journal();
        assert!(j
            .consume(&event("initial", 1, 2, "SAVE_RETURNED", "returned"))
            .is_err());
        let mut j = journal();
        j.consume(&event("initial", 1, 1, "SAVE_RETURNED", "real"))
            .unwrap();
        assert!(j
            .consume(&event("initial", 2, 1, "SAVE_COMPLETE", "changed"))
            .is_err());
        let mut j = journal();
        assert!(j
            .consume(&event("initial", 1, 1, "EXIT_REQUESTED", "exit"))
            .is_err());
        let f = event("initial", 1, 0, "BOOT", "real");
        let mut altered = f.clone();
        altered.body = serde_json::json!({"generation":1,"canonical":"real"}).to_string();
        assert!(altered.payload().is_err());
    }
}
