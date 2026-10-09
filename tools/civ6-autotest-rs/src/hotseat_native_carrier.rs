//! Strict, separate log contract for the native-carrier save fixture.
//! The original growth witness keeps its 21-step/49-field contract unchanged.
use crate::{Error, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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

#[cfg(test)]
mod tests {
    use super::*;
    const NONCE: &str = "0123456789abcdef0123456789abcdef";
    fn packet(serial: u64, part: usize, total: usize, body: &str) -> String {
        let hex: String = body.bytes().map(|b| format!("{b:02x}")).collect();
        format!("witness: JAC_NC|1|{NONCE}|12.5|{serial}|1|25|RESULT|{part}/{total}|{hex}")
    }

    #[test]
    fn native_steps_above_the_unchanged_growth_limit_and_utf8_are_retained() {
        let frames = parse_available_frames(&packet(1, 1, 1, "原本"), NONCE).unwrap();
        assert_eq!(frames[0].step, 25);
        assert_eq!(frames[0].body, "原本");
        let old = format!("JAC_HS|1|{NONCE}|12.5|1|21|RESULT|1/1|e58e9fe69cac");
        assert_eq!(
            civ6_autotest_rs::hotseat_bridge::parse_frames(&old, NONCE).unwrap()[0].body,
            "原本"
        );
        assert!(civ6_autotest_rs::hotseat_bridge::parse_frames(
            &old.replace("|21|", "|25|"),
            NONCE
        )
        .is_err());
    }

    #[test]
    fn partial_delivery_is_not_success_and_duplicates_gaps_and_interleaving_fail() {
        let first = packet(1, 1, 2, "source");
        assert!(parse_available_frames(&first, NONCE).unwrap().is_empty());
        let good = format!("{first}\n{}", packet(1, 2, 2, " returned"));
        assert_eq!(
            parse_available_frames(&good, NONCE).unwrap()[0].body,
            "source returned"
        );
        for tail in [
            first.clone(),
            packet(1, 3, 3, "missing"),
            packet(2, 2, 2, "interleaved"),
        ] {
            assert!(parse_available_frames(&format!("{first}\n{tail}"), NONCE).is_err());
        }
    }

    #[test]
    fn foreign_nonce_invalid_utf8_and_unbounded_headers_are_rejected() {
        let good = packet(1, 1, 1, "real");
        for bad in [
            good.replace(NONCE, "ffffffffffffffffffffffffffffffff"),
            good.replace("|1|25|", "|3|25|"),
            good.replace("|25|", "|44|"),
            good.replace("|1/1|7265616c", "|1/1|ff"),
            good.replace("|1/1|", "|1/10924|"),
        ] {
            assert!(parse_available_frames(&bad, NONCE).is_err(), "{bad}");
        }
    }

    #[test]
    fn receipt_exact_repoll_is_harmless_but_replays_and_body_changes_fail() {
        let mut receipts = Receipts::default();
        let first = parse_available_frames(&packet(1, 1, 1, "one"), NONCE)
            .unwrap()
            .remove(0);
        assert!(receipts.accept(&first).unwrap());
        assert!(!receipts.accept(&first).unwrap());
        let mut changed = first.clone();
        changed.body = "forged".into();
        assert!(receipts.accept(&changed).is_err());
        let second = parse_available_frames(&packet(2, 1, 1, "two"), NONCE)
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
        assert!(parse_available_frames(&log, NONCE).is_err());
        let mut receipts = Receipts::default();
        let mut frame = parse_available_frames(&packet(1, 1, 1, "real"), NONCE)
            .unwrap()
            .remove(0);
        receipts.accept(&frame).unwrap();
        frame.serial = u64::MAX;
        assert!(receipts.accept(&frame).is_err());
    }
}
