//! Synchronous, one-shot FireTuner commands over the IPv4 loopback interface.
//!
//! A failed query is a preflight failure. Once a Lua write has been attempted,
//! transport failures are unknown outcomes; this client never reconnects or
//! replays them. Each public operation has one deadline, including partial I/O.

use sha2::{Digest, Sha256};
use std::error::Error;
use std::fmt;
use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, Shutdown, SocketAddr, TcpStream};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const MAX_FRAME: usize = 2 * 1024 * 1024;
const MAX_OUTPUT: usize = 4 * MAX_FRAME;
const HANDSHAKE: i32 = 4;
const COMMAND: i32 = 3;
static TOKEN_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug)]
pub enum TunerError {
    Preflight(String),
    Unknown(String),
    Lua(String),
}

impl fmt::Display for TunerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Preflight(message) => write!(f, "FireTuner preflight failed: {message}"),
            Self::Unknown(message) => write!(
                f,
                "Lua execution is unknown; the command may have run. Do not replay it: {message}"
            ),
            Self::Lua(message) => write!(f, "Lua execution failed: {message}"),
        }
    }
}

impl Error for TunerError {}

#[derive(Debug)]
pub struct LuaResult {
    pub value: String,
    pub output: String,
}

pub struct TunerClient {
    stream: Option<TcpStream>,
}

impl TunerClient {
    /// Connect only to 127.0.0.1 and complete APP/LSQ under the same deadline.
    pub fn connect(port: u16, timeout: Duration) -> Result<Self, TunerError> {
        let deadline = deadline(timeout).map_err(TunerError::Preflight)?;
        if port == 0 {
            return Err(TunerError::Preflight("port must be nonzero".into()));
        }
        let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
        let stream = TcpStream::connect_timeout(
            &address,
            remaining(deadline).map_err(TunerError::Preflight)?,
        )
        .map_err(|e| TunerError::Preflight(format!("cannot connect to {address}: {e}")))?;
        stream
            .set_nodelay(true)
            .map_err(|e| TunerError::Preflight(format!("cannot configure connection: {e}")))?;
        let mut client = Self {
            stream: Some(stream),
        };
        let identity = client
            .query("APP:", deadline)
            .map_err(TunerError::Preflight)?;
        if identity.trim().is_empty() {
            return Err(TunerError::Preflight("APP returned no identity".into()));
        }
        client.state_pairs_until(deadline)?;
        remaining(deadline).map_err(TunerError::Preflight)?;
        Ok(client)
    }

    pub fn list_state_pairs(
        &mut self,
        timeout: Duration,
    ) -> Result<Vec<(i32, String)>, TunerError> {
        let deadline = deadline(timeout).map_err(TunerError::Preflight)?;
        self.state_pairs_until(deadline)
    }

    pub fn run_lua(
        &mut self,
        state: &str,
        code: &str,
        timeout: Duration,
    ) -> Result<LuaResult, TunerError> {
        let deadline = deadline(timeout).map_err(TunerError::Preflight)?;
        if state.trim().is_empty() || state.contains('\0') {
            return Err(TunerError::Preflight("invalid Lua state name".into()));
        }
        let token = new_token();
        // Reserve the longest possible i32 index, before sending even the LSQ query.
        let overhead = lua_payload(i32::MIN, "", &token).len() + 1;
        if code.contains('\0') || code.len() > MAX_FRAME - overhead {
            return Err(TunerError::Preflight(
                "Lua code contains NUL or exceeds the 2 MiB command limit".into(),
            ));
        }
        let pairs = self.state_pairs_until(deadline)?;
        let wanted = state.to_lowercase();
        let matches: Vec<_> = pairs
            .iter()
            .filter(|(_, name)| name.to_lowercase() == wanted)
            .collect();
        if matches.len() != 1 {
            return Err(TunerError::Preflight(format!(
                "Lua state {state:?} is not uniquely available; states: {pairs:?}"
            )));
        }
        let wire = encode_frame(COMMAND, &lua_payload(matches[0].0, code, &token))
            .map_err(TunerError::Preflight)?;
        let stream = self.stream.as_mut().ok_or_else(|| {
            TunerError::Preflight(
                "connection is closed; connect explicitly before a new command".into(),
            )
        })?;
        let sent = write_packet(stream, &wire, deadline, |stream, timeout| {
            stream.set_write_timeout(Some(timeout))
        });
        if let Err(failure) = sent {
            self.invalidate();
            return Err(if failure.attempted {
                TunerError::Unknown(failure.message)
            } else {
                TunerError::Preflight(failure.message)
            });
        }
        let result = self.await_ack(state, &token, deadline);
        if matches!(&result, Err(TunerError::Unknown(_))) {
            self.invalidate();
        }
        result
    }

    fn state_pairs_until(&mut self, deadline: Instant) -> Result<Vec<(i32, String)>, TunerError> {
        let raw = self
            .query("LSQ:", deadline)
            .map_err(TunerError::Preflight)?;
        let pairs = parse_state_pairs(&raw).map_err(TunerError::Preflight)?;
        remaining(deadline).map_err(TunerError::Preflight)?;
        Ok(pairs)
    }

    fn query(&mut self, payload: &str, deadline: Instant) -> Result<String, String> {
        let wire = encode_frame(HANDSHAKE, payload)?;
        let result = (|| {
            let stream = self
                .stream
                .as_mut()
                .ok_or_else(|| "FireTuner connection is closed".to_string())?;
            write_packet(stream, &wire, deadline, |stream, timeout| {
                stream.set_write_timeout(Some(timeout))
            })
            .map_err(|failure| failure.message)?;
            loop {
                let frame = recv_frame(stream, deadline)?;
                if frame.payload.is_empty()
                    || frame.payload == "O"
                    || frame.payload.starts_with("O ")
                    || frame.payload.starts_with("O\0")
                {
                    continue;
                }
                // The legacy client accepts reply tags independently of the
                // outgoing command tag; only output/empty frames are excluded.
                return Ok(frame.payload);
            }
        })();
        // A partial query/reply must not be reused as the next query's reply.
        if result.is_err() {
            self.invalidate();
        }
        result
    }

    fn await_ack(
        &mut self,
        state: &str,
        token: &str,
        deadline: Instant,
    ) -> Result<LuaResult, TunerError> {
        let stream = self
            .stream
            .as_mut()
            .ok_or_else(|| TunerError::Unknown("connection disappeared after Lua send".into()))?;
        let mut output = String::new();
        loop {
            let frame = recv_frame(stream, deadline).map_err(TunerError::Unknown)?;
            if frame.payload.is_empty() {
                continue;
            }
            if output.len() + frame.payload.len() + 1 > MAX_OUTPUT {
                return Err(TunerError::Unknown(
                    "Lua output exceeds the bounded buffer".into(),
                ));
            }
            if !output.is_empty() {
                output.push('\n');
            }
            output.push_str(&frame.payload);
            if let Some((ok, value)) =
                acknowledgement(&frame.payload, token).map_err(TunerError::Unknown)?
            {
                remaining(deadline).map_err(TunerError::Unknown)?;
                if !ok {
                    return Err(TunerError::Lua(format!("in {state}: {value}")));
                }
                return Ok(LuaResult { value, output });
            }
        }
    }

    fn invalidate(&mut self) {
        if let Some(stream) = self.stream.take() {
            let _ = stream.shutdown(Shutdown::Both);
        }
    }
}

fn deadline(timeout: Duration) -> Result<Instant, String> {
    if timeout.is_zero() {
        return Err("timeout must be positive".into());
    }
    Instant::now()
        .checked_add(timeout)
        .ok_or_else(|| "timeout is out of range".into())
}

fn remaining(deadline: Instant) -> Result<Duration, String> {
    deadline
        .checked_duration_since(Instant::now())
        .filter(|left| !left.is_zero())
        .ok_or_else(|| "operation deadline expired".into())
}

fn encode_frame(tag: i32, payload: &str) -> Result<Vec<u8>, String> {
    if payload.contains('\0') || payload.len() >= MAX_FRAME {
        return Err("outgoing frame contains NUL or exceeds 2 MiB".into());
    }
    let mut wire = Vec::with_capacity(8 + payload.len() + 1);
    wire.extend_from_slice(&((payload.len() + 1) as u32).to_le_bytes());
    wire.extend_from_slice(&tag.to_le_bytes());
    wire.extend_from_slice(payload.as_bytes());
    wire.push(0);
    Ok(wire)
}

#[derive(Debug)]
struct SendFailure {
    message: String,
    attempted: bool,
}

fn write_packet<W: Write>(
    writer: &mut W,
    wire: &[u8],
    deadline: Instant,
    mut set_timeout: impl FnMut(&W, Duration) -> io::Result<()>,
) -> Result<(), SendFailure> {
    let mut attempted = false;
    let mut written = 0;
    while written < wire.len() {
        let left = remaining(deadline).map_err(|message| SendFailure { message, attempted })?;
        set_timeout(writer, left).map_err(|e| SendFailure {
            message: format!(
                "cannot configure write after {written}/{} bytes: {e}",
                wire.len()
            ),
            attempted,
        })?;
        // Even a failed first write might have reached the peer. Never retry the packet.
        attempted = true;
        match writer.write(&wire[written..]) {
            Ok(0) => {
                return Err(SendFailure {
                    message: format!("connection closed after {written}/{} bytes", wire.len()),
                    attempted,
                });
            }
            Ok(count) => written += count,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => {
                return Err(SendFailure {
                    message: format!("write failed after {written}/{} bytes: {e}", wire.len()),
                    attempted,
                });
            }
        }
    }
    remaining(deadline).map_err(|message| SendFailure { message, attempted })?;
    Ok(())
}

struct Frame {
    _tag: i32,
    payload: String,
}

fn read_exact_until(
    stream: &mut TcpStream,
    bytes: &mut [u8],
    deadline: Instant,
) -> Result<(), String> {
    let mut read = 0;
    while read < bytes.len() {
        stream
            .set_read_timeout(Some(remaining(deadline)?))
            .map_err(|e| format!("cannot configure read: {e}"))?;
        match stream.read(&mut bytes[read..]) {
            Ok(0) => {
                return Err(format!(
                    "connection closed after {read}/{} bytes",
                    bytes.len()
                ));
            }
            Ok(count) => read += count,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => {
                return Err(format!(
                    "read failed after {read}/{} bytes: {e}",
                    bytes.len()
                ));
            }
        }
    }
    remaining(deadline)?;
    Ok(())
}

fn recv_frame(stream: &mut TcpStream, deadline: Instant) -> Result<Frame, String> {
    let mut header = [0u8; 8];
    read_exact_until(stream, &mut header, deadline)?;
    let size = u32::from_le_bytes(header[..4].try_into().expect("four-byte size")) as usize;
    let tag = i32::from_le_bytes(header[4..].try_into().expect("four-byte tag"));
    if size > MAX_FRAME {
        return Err(format!("invalid FireTuner frame size: {size}"));
    }
    if size == 0 {
        return Ok(Frame {
            _tag: tag,
            payload: String::new(),
        });
    }
    let mut body = vec![0; size];
    read_exact_until(stream, &mut body, deadline)?;
    if body.last() != Some(&0) {
        return Err("FireTuner payload is not NUL terminated".into());
    }
    while body.last() == Some(&0) {
        body.pop();
    }
    let payload = String::from_utf8(body).map_err(|e| format!("invalid FireTuner UTF-8: {e}"))?;
    Ok(Frame { _tag: tag, payload })
}

fn parse_state_pairs(raw: &str) -> Result<Vec<(i32, String)>, String> {
    let raw = raw.strip_prefix("LSQ:").unwrap_or(raw);
    let fields: Vec<_> = raw
        .split(['\r', '\n', '\0'])
        .map(str::trim)
        .filter(|field| !field.is_empty())
        .collect();
    if fields.is_empty() || fields.len() % 2 != 0 {
        return Err("LSQ returned no states or incomplete index/name pairs".into());
    }
    let mut pairs = Vec::with_capacity(fields.len() / 2);
    for pair in fields.chunks_exact(2) {
        let index: i32 = pair[0]
            .parse()
            .map_err(|_| format!("LSQ state index is not an i32: {:?}", pair[0]))?;
        if pairs.iter().any(|(old, _)| *old == index) {
            return Err(format!("LSQ returned duplicate state index {index}"));
        }
        pairs.push((index, pair[1].to_string()));
    }
    Ok(pairs)
}

fn new_token() -> String {
    let mut hash = Sha256::new();
    let time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    hash.update(time.as_nanos().to_le_bytes());
    hash.update(std::process::id().to_le_bytes());
    hash.update(TOKEN_SEQUENCE.fetch_add(1, Ordering::Relaxed).to_le_bytes());
    let digest = hash.finalize();
    let mut token = String::from("JAC_AUTOTEST_");
    for byte in &digest[..16] {
        use std::fmt::Write as _;
        write!(&mut token, "{byte:02x}").expect("writing a String cannot fail");
    }
    token
}

fn lua_payload(index: i32, code: &str, token: &str) -> String {
    format!(
        "CMD:{index}:local __ok, __value = pcall(function()\n{code}\nend)\nprint(\"{token}|\" .. (__ok and \"OK|\" or \"ERR|\") .. tostring(__value))"
    )
}

fn acknowledgement(payload: &str, token: &str) -> Result<Option<(bool, String)>, String> {
    let marker = format!("{token}|");
    let mut found = None;
    for line in payload.lines() {
        for (index, _) in line.match_indices(marker.as_str()) {
            let suffix = &line[index + marker.len()..];
            let Some((status, value)) = suffix.split_once('|') else {
                continue;
            };
            let ok = match status {
                "OK" => true,
                "ERR" => false,
                _ => continue,
            };
            if found.is_some() {
                return Err("multiple acknowledgments for the one-shot Lua command".into());
            }
            found = Some((ok, value.to_string()));
        }
    }
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;
    use regex::Regex;
    use std::net::TcpListener;
    use std::thread::{self, JoinHandle};

    const STATES: &str = "3\x00FrontEnd\x0017\x00InGame\x0041\x00GameCore_Tuner";

    #[derive(Clone, Copy)]
    enum Reply {
        Ok,
        LuaError,
        NoAck,
        Disconnect,
        PartialAck,
        DuplicateAck,
        SlowQueryAndAck,
    }

    // Fixture framing is written separately from the implementation under test.
    fn fixture_frame(stream: &mut TcpStream, tag: i32, payload: &str) {
        let mut wire = Vec::new();
        wire.extend(((payload.len() + 1) as u32).to_le_bytes());
        wire.extend(tag.to_le_bytes());
        wire.extend(payload.bytes());
        wire.push(0);
        for part in [0..3, 3..9, 9..wire.len()] {
            stream.write_all(&wire[part]).unwrap();
        }
    }

    fn fixture_read(stream: &mut TcpStream) -> io::Result<(i32, String)> {
        let mut header = [0; 8];
        stream.read_exact(&mut header)?;
        let size = u32::from_le_bytes(header[..4].try_into().unwrap()) as usize;
        assert!(size <= MAX_FRAME);
        let tag = i32::from_le_bytes(header[4..].try_into().unwrap());
        let mut body = vec![0; size];
        stream.read_exact(&mut body)?;
        assert_eq!(body.pop(), Some(0));
        Ok((tag, String::from_utf8(body).unwrap()))
    }

    fn fake(states: &str, reply: Reply) -> (u16, JoinHandle<Vec<(i32, String)>>) {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let states = states.to_string();
        let handle = thread::spawn(move || {
            let (mut stream, peer) = listener.accept().unwrap();
            assert!(peer.ip().is_loopback());
            stream
                .set_read_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            stream
                .set_write_timeout(Some(Duration::from_secs(2)))
                .unwrap();
            let mut received = Vec::new();
            let re = Regex::new(r"JAC_AUTOTEST_[0-9a-f]{32}").unwrap();
            let mut state_queries = 0;
            while let Ok((tag, payload)) = fixture_read(&mut stream) {
                received.push((tag, payload.clone()));
                match payload.as_str() {
                    "APP:" => {
                        assert_eq!(tag, HANDSHAKE);
                        stream.write_all(&[0, 0, 0, 0, 3, 0, 0, 0]).unwrap();
                        fixture_frame(&mut stream, COMMAND, "O asynchronous print");
                        fixture_frame(&mut stream, HANDSHAKE, "Civilization VI");
                    }
                    "LSQ:" => {
                        assert_eq!(tag, HANDSHAKE);
                        state_queries += 1;
                        if matches!(reply, Reply::SlowQueryAndAck) && state_queries > 1 {
                            thread::sleep(Duration::from_millis(100));
                        }
                        fixture_frame(&mut stream, COMMAND, "O\0another print");
                        fixture_frame(&mut stream, HANDSHAKE, &states);
                    }
                    _ => {
                        assert_eq!(tag, COMMAND);
                        let token = re.find(&payload).unwrap().as_str();
                        match reply {
                            Reply::NoAck => {
                                thread::sleep(Duration::from_millis(350));
                                break;
                            }
                            Reply::Disconnect => break,
                            Reply::PartialAck => {
                                stream.write_all(&[100, 0, 0]).unwrap();
                                break;
                            }
                            Reply::DuplicateAck => fixture_frame(
                                &mut stream,
                                COMMAND,
                                &format!("{token}|OK|one\n{token}|OK|two"),
                            ),
                            Reply::SlowQueryAndAck => {
                                thread::sleep(Duration::from_millis(150));
                                let body = format!("{token}|OK|true\0").into_bytes();
                                let wire = [
                                    (body.len() as u32).to_le_bytes().as_slice(),
                                    COMMAND.to_le_bytes().as_slice(),
                                    body.as_slice(),
                                ]
                                .concat();
                                let _ = stream.write_all(&wire);
                                break;
                            }
                            Reply::LuaError => {
                                fixture_frame(&mut stream, COMMAND, &format!("O {token}|ERR|boom"))
                            }
                            Reply::Ok => {
                                fixture_frame(&mut stream, COMMAND, "unrelated|OK|old");
                                fixture_frame(&mut stream, COMMAND, "O printed 日本語");
                                fixture_frame(&mut stream, COMMAND, &format!("O {token}|OK|true"));
                            }
                        }
                    }
                }
            }
            received
        });
        (port, handle)
    }

    fn count_commands(received: &[(i32, String)]) -> usize {
        received.iter().filter(|(tag, _)| *tag == COMMAND).count()
    }

    #[test]
    fn fragmented_frames_skip_async_queries_and_use_actual_index() {
        let (port, server) = fake(STATES, Reply::Ok);
        let mut client = TunerClient::connect(port, Duration::from_secs(1)).unwrap();
        assert_eq!(
            client.list_state_pairs(Duration::from_secs(1)).unwrap(),
            vec![
                (3, "FrontEnd".into()),
                (17, "InGame".into()),
                (41, "GameCore_Tuner".into()),
            ]
        );
        let result = client
            .run_lua("ingame", "return true", Duration::from_secs(1))
            .unwrap();
        assert_eq!(result.value, "true");
        assert!(result.output.contains("printed 日本語"));
        drop(client);
        let received = server.join().unwrap();
        assert_eq!(count_commands(&received), 1);
        assert!(
            received
                .iter()
                .any(|(tag, code)| *tag == COMMAND && code.starts_with("CMD:17:"))
        );
    }

    #[test]
    fn confirmed_err_is_lua_not_unknown() {
        let (port, server) = fake(STATES, Reply::LuaError);
        let mut client = TunerClient::connect(port, Duration::from_secs(1)).unwrap();
        let result = client.run_lua("InGame", "error('boom')", Duration::from_secs(1));
        assert!(matches!(result, Err(TunerError::Lua(ref message)) if message.contains("boom")));
        drop(client);
        assert_eq!(count_commands(&server.join().unwrap()), 1);
    }

    #[test]
    fn timeout_disconnect_partial_reply_and_duplicate_ack_never_replay() {
        for reply in [
            Reply::NoAck,
            Reply::Disconnect,
            Reply::PartialAck,
            Reply::DuplicateAck,
        ] {
            let (port, server) = fake(STATES, reply);
            let mut client = TunerClient::connect(port, Duration::from_secs(1)).unwrap();
            assert!(matches!(
                client.run_lua("InGame", "return true", Duration::from_millis(150)),
                Err(TunerError::Unknown(_))
            ));
            assert!(matches!(
                client.run_lua("InGame", "return true", Duration::from_secs(1)),
                Err(TunerError::Preflight(_))
            ));
            drop(client);
            assert_eq!(count_commands(&server.join().unwrap()), 1);
        }
    }

    #[test]
    fn absent_and_ambiguous_states_fail_before_lua_write() {
        for states in ["3\x00FrontEnd", "17\x00InGame\x0041\x00INGAME"] {
            let (port, server) = fake(states, Reply::Ok);
            let mut client = TunerClient::connect(port, Duration::from_secs(1)).unwrap();
            assert!(matches!(
                client.run_lua("InGame", "return true", Duration::from_secs(1)),
                Err(TunerError::Preflight(_))
            ));
            drop(client);
            assert_eq!(count_commands(&server.join().unwrap()), 0);
        }
    }

    #[test]
    fn invalid_code_fails_before_queries_or_commands() {
        let (port, server) = fake(STATES, Reply::Ok);
        let mut client = TunerClient::connect(port, Duration::from_secs(1)).unwrap();
        for code in ["return\0true".to_string(), "x".repeat(MAX_FRAME)] {
            assert!(matches!(
                client.run_lua("InGame", &code, Duration::from_secs(1)),
                Err(TunerError::Preflight(_))
            ));
        }
        drop(client);
        let received = server.join().unwrap();
        assert_eq!(count_commands(&received), 0);
        assert_eq!(received.len(), 2);
    }

    #[test]
    fn lsq_requires_complete_explicit_indexes() {
        for raw in [
            "",
            "17\x00InGame\x0041",
            "FrontEnd\0InGame",
            "2147483648\0InGame",
            "17\x00InGame\x0017\x00Other",
        ] {
            assert!(parse_state_pairs(raw).is_err(), "{raw:?}");
        }
        assert_eq!(
            parse_state_pairs("LSQ:3\r\nFrontEnd\r\n17\r\nInGame").unwrap(),
            vec![(3, "FrontEnd".into()), (17, "InGame".into())]
        );
    }

    #[test]
    fn partial_write_remembers_attempt_and_does_not_restart_packet() {
        struct PartialWriter {
            bytes: Vec<u8>,
            calls: usize,
        }
        impl Write for PartialWriter {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                self.calls += 1;
                if self.calls == 1 {
                    self.bytes.extend_from_slice(&bytes[..3]);
                    Ok(3)
                } else {
                    Err(io::Error::new(
                        io::ErrorKind::BrokenPipe,
                        "fixture partial write",
                    ))
                }
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let wire = encode_frame(COMMAND, "CMD:17:return true").unwrap();
        let mut writer = PartialWriter {
            bytes: Vec::new(),
            calls: 0,
        };
        let error = write_packet(
            &mut writer,
            &wire,
            Instant::now() + Duration::from_secs(1),
            |_, _| Ok(()),
        )
        .unwrap_err();
        assert!(error.attempted);
        assert_eq!(writer.calls, 2);
        assert_eq!(writer.bytes, wire[..3]);
        let mut untouched = Vec::new();
        let error = write_packet(
            &mut untouched,
            &wire,
            Instant::now() - Duration::from_secs(1),
            |_, _| Ok(()),
        )
        .unwrap_err();
        assert!(!error.attempted);
        assert!(untouched.is_empty());
    }

    #[test]
    fn frame_limit_is_bytes_and_includes_nul() {
        let wire = encode_frame(3, "日本").unwrap();
        assert_eq!(&wire[..8], &[7, 0, 0, 0, 3, 0, 0, 0]);
        assert_eq!(&wire[8..], "日本\0".as_bytes());
        assert!(encode_frame(3, &"x".repeat(MAX_FRAME - 1)).is_ok());
        assert!(encode_frame(3, &"x".repeat(MAX_FRAME)).is_err());
        assert!(encode_frame(3, "bad\0code").is_err());
    }

    #[test]
    fn oversized_unterminated_and_invalid_utf8_incoming_frames_fail() {
        for wire in [
            vec![1, 0, 32, 0, 3, 0, 0, 0],
            vec![1, 0, 0, 0, 3, 0, 0, 0, b'x'],
            vec![2, 0, 0, 0, 3, 0, 0, 0, 255, 0],
        ] {
            let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
            let address = listener.local_addr().unwrap();
            let server = thread::spawn(move || {
                let (mut peer, _) = listener.accept().unwrap();
                peer.write_all(&wire).unwrap();
            });
            let mut stream = TcpStream::connect(address).unwrap();
            assert!(recv_frame(&mut stream, Instant::now() + Duration::from_secs(1)).is_err());
            server.join().unwrap();
        }
    }

    #[test]
    fn acknowledgment_requires_this_token_and_unambiguous_status() {
        assert_eq!(acknowledgement("other|OK|yes", "wanted").unwrap(), None);
        assert_eq!(
            acknowledgement("wanted|pending|yes", "wanted").unwrap(),
            None
        );
        assert_eq!(
            acknowledgement("O wanted|OK|a|b", "wanted").unwrap(),
            Some((true, "a|b".into()))
        );
        assert!(acknowledgement("wanted|OK|one\nwanted|ERR|two", "wanted").is_err());
        assert_ne!(new_token(), new_token());
    }

    #[test]
    fn connect_handshakes_share_one_deadline() {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(1)))
                .unwrap();
            assert_eq!(
                fixture_read(&mut stream).unwrap(),
                (HANDSHAKE, "APP:".into())
            );
            thread::sleep(Duration::from_millis(100));
            fixture_frame(&mut stream, HANDSHAKE, "Civilization VI");
            assert_eq!(
                fixture_read(&mut stream).unwrap(),
                (HANDSHAKE, "LSQ:".into())
            );
            thread::sleep(Duration::from_millis(150));
            // A client that grants LSQ a new timeout would incorrectly succeed.
            // The correct client may already have closed the connection.
            let body = format!("{STATES}\0").into_bytes();
            let wire = [
                (body.len() as u32).to_le_bytes().as_slice(),
                HANDSHAKE.to_le_bytes().as_slice(),
                body.as_slice(),
            ]
            .concat();
            let _ = stream.write_all(&wire);
        });
        let started = Instant::now();
        assert!(matches!(
            TunerClient::connect(port, Duration::from_millis(200)),
            Err(TunerError::Preflight(_))
        ));
        assert!(started.elapsed() < Duration::from_millis(350));
        server.join().unwrap();
    }

    #[test]
    fn zero_timeout_or_port_is_preflight() {
        assert!(matches!(
            TunerClient::connect(0, Duration::from_secs(1)),
            Err(TunerError::Preflight(_))
        ));
        assert!(matches!(
            TunerClient::connect(0, Duration::ZERO),
            Err(TunerError::Preflight(_))
        ));
    }

    #[test]
    fn state_query_and_lua_ack_share_one_deadline() {
        let (port, server) = fake(STATES, Reply::SlowQueryAndAck);
        let mut client = TunerClient::connect(port, Duration::from_secs(1)).unwrap();
        assert!(matches!(
            client.run_lua("InGame", "return true", Duration::from_millis(200)),
            Err(TunerError::Unknown(_))
        ));
        drop(client);
        assert_eq!(count_commands(&server.join().unwrap()), 1);
    }
}
