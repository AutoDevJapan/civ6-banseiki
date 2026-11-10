//! A bounded exit-recovery notification. It requests the supervisor's existing
//! owned cleanup protocol; its numbers never grant process-control authority.
pub const REQUIRED: &str = "CIV6_NATIVE_RECOVERY_REQUIRED_V1";
pub const NONCE_ENV: &str = "CIV6_NATIVE_RECOVERY_NONCE";
pub const MAX_LINE: usize = 256;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Target {
    pub pid: u32,
    pub created: u64,
}
pub fn valid_nonce(nonce: &str) -> bool {
    !nonce.is_empty()
        && nonce.len() <= 64
        && nonce.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-')
}
pub fn format_notice(nonce: &str, target: Target) -> Option<String> {
    (valid_nonce(nonce) && target.pid != 0 && target.created != 0)
        .then(|| format!("{REQUIRED} {nonce} {} {}\n", target.pid, target.created))
}
pub fn parse_notice(line: &[u8], nonce: &str) -> Result<Option<Target>, &'static str> {
    if !valid_nonce(nonce) || line.len() > MAX_LINE {
        return Ok(None);
    }
    let line = line.strip_suffix(b"\n").unwrap_or(line);
    let line = line.strip_suffix(b"\r").unwrap_or(line);
    let prefix = format!("{REQUIRED} {nonce} ");
    let Some(payload) = line.strip_prefix(prefix.as_bytes()) else {
        return Ok(None);
    };
    let payload = std::str::from_utf8(payload).map_err(|_| "invalid native recovery identity")?;
    let fields = payload.split(' ').collect::<Vec<_>>();
    if fields.len() != 2 {
        return Err("invalid native recovery identity");
    }
    let pid = fields[0]
        .parse::<u32>()
        .map_err(|_| "invalid native recovery PID")?;
    let created = fields[1]
        .parse::<u64>()
        .map_err(|_| "invalid native recovery creation time")?;
    if pid == 0 || created == 0 {
        return Err("empty native recovery identity");
    }
    Ok(Some(Target { pid, created }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn notification_roundtrips_only_the_bound_nonce_and_exact_identity() {
        let target = Target {
            pid: 42,
            created: 71,
        };
        let line = format_notice("123-abc-def", target).unwrap();
        assert!(line.len() <= MAX_LINE);
        assert_eq!(
            parse_notice(line.as_bytes(), "123-abc-def"),
            Ok(Some(target))
        );
        assert_eq!(parse_notice(line.as_bytes(), "123-abc-eee"), Ok(None));
        assert!(parse_notice(format!("{REQUIRED} abc 0 71").as_bytes(), "abc").is_err());
    }
    #[test]
    fn missing_invalid_and_oversized_nonce_never_produce_a_notification() {
        for nonce in ["", "x-not-hex", "aa\nbb", &"a".repeat(65)] {
            assert!(format_notice(
                nonce,
                Target {
                    pid: 42,
                    created: 71
                }
            )
            .is_none());
        }
        assert!(format_notice(
            "abc",
            Target {
                pid: 0,
                created: 71
            }
        )
        .is_none());
        assert!(format_notice(
            "abc",
            Target {
                pid: 42,
                created: 0
            }
        )
        .is_none());
    }
}
