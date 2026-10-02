//! Synchronous FireTuner transport. A Lua command is sent at most once.

use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, Shutdown, SocketAddr, SocketAddrV4, TcpStream};
use std::time::Duration;

use crate::error::{Error, Result};
use crate::time::monotonic;
use caseless::Caseless;
pub fn state_name_eq(a: &str, b: &str) -> bool {
    a.chars().default_caseless_match(b.chars())
}

pub const PORT: u16 = 4318;
/// Parse an integer CLI argument without imposing the socket port range until use.
#[derive(Clone, Debug)]
pub struct PortArgument(Option<u16>);
impl std::str::FromStr for PortArgument {
    type Err = String;
    fn from_str(value: &str) -> std::result::Result<Self, Self::Err> {
        let value = value.trim();
        let digits = value.strip_prefix(['+', '-']).unwrap_or(value);
        let parts: Vec<_> = digits.split('_').collect();
        if parts
            .iter()
            .any(|part| part.is_empty() || !part.bytes().all(|b| b.is_ascii_digit()))
        {
            return Err(format!("invalid integer value: {value:?}"));
        }
        let normalized = value.replace('_', "");
        Ok(Self(normalized.parse().ok().filter(|port| *port != 0)))
    }
}
impl PortArgument {
    pub fn socket_port(&self) -> Result<u16> {
        self.0
            .ok_or_else(|| Error::input("Invalid port or timeout"))
    }
}
pub const MAX_FRAME: usize = 2 * 1024 * 1024;
pub const HANDSHAKE: i32 = 4;
pub const COMMAND: i32 = 3;

const LUA_PREFIX: &str = "local __ok, __value = pcall(function()\n";
const LUA_MIDDLE: &str = "\nend)\n";
const LUA_PRINT_PREFIX: &str = "print(\"";
const LUA_PRINT_SUFFIX: &str = "|\" .. (__ok and \"OK|\" or \"ERR|\") .. tostring(__value))";
const TOKEN_PREFIX: &str = "JAC_AUTOTEST_";

// The largest generated command reserves a 20-digit state index, UUID token, and NUL.
pub const MAX_LUA_CODE_BYTES: usize = MAX_FRAME
    - ("CMD:".len()
        + 20
        + ":".len()
        + LUA_PREFIX.len()
        + LUA_MIDDLE.len()
        + LUA_PRINT_PREFIX.len()
        + TOKEN_PREFIX.len()
        + 32
        + LUA_PRINT_SUFFIX.len())
    - 1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Frame {
    pub tag: i32,
    pub payload: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LuaResult {
    pub value: String,
    pub output: String,
}

pub fn validate_lua_code(code: &str) -> Result<()> {
    if code.contains('\0') {
        return Err(Error::input("NUL is not allowed in Lua code"));
    }
    if code.len() > MAX_LUA_CODE_BYTES {
        return Err(Error::input(
            "Lua code is too large for a FireTuner command",
        ));
    }
    Ok(())
}

pub fn lua_payload(index: i64, code: &str, token: &str) -> String {
    format!("CMD:{index}:{LUA_PREFIX}{code}{LUA_MIDDLE}{LUA_PRINT_PREFIX}{token}{LUA_PRINT_SUFFIX}")
}

fn frame_bytes(tag: i32, payload: &str) -> Result<Vec<u8>> {
    if payload.contains('\0') {
        return Err(Error::input(
            "NUL is not allowed in an outgoing FireTuner command",
        ));
    }
    let size = payload
        .len()
        .checked_add(1)
        .ok_or_else(|| Error::input("FireTuner command is too large"))?;
    if size > MAX_FRAME {
        return Err(Error::input("FireTuner command is too large"));
    }
    let mut bytes = Vec::with_capacity(8 + size);
    bytes.extend_from_slice(&(size as u32).to_le_bytes());
    bytes.extend_from_slice(&tag.to_le_bytes());
    bytes.extend_from_slice(payload.as_bytes());
    bytes.push(0);
    Ok(bytes)
}

fn timeout_duration(deadline: f64) -> Result<Duration> {
    let remaining = deadline - monotonic();
    if remaining <= 0.0 || !remaining.is_finite() {
        return Err(Error::tuner("FireTuner deadline expired"));
    }
    // Socket timeouts reject zero; one microsecond is the smallest useful wait here.
    Ok(Duration::from_secs_f64(remaining.max(0.000_001)))
}

fn write_bytes(stream: &mut TcpStream, bytes: &[u8], deadline: Option<f64>) -> Result<()> {
    let mut written = 0;
    while written < bytes.len() {
        let timeout = deadline.map(timeout_duration).transpose()?;
        stream
            .set_write_timeout(timeout)
            .map_err(|e| Error::tuner(e.to_string()))?;
        match stream.write(&bytes[written..]) {
            Ok(0) => return Err(Error::tuner("FireTuner closed the connection during send")),
            Ok(count) => written += count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(Error::tuner(error.to_string())),
        }
    }
    Ok(())
}

/// Encode and send a FireTuner frame. Client operations use an absolute deadline internally.
pub fn send_frame(stream: &mut TcpStream, tag: i32, payload: &str) -> Result<()> {
    let bytes = frame_bytes(tag, payload)?;
    write_bytes(stream, &bytes, None)
}

fn read_exact(stream: &mut TcpStream, count: usize, deadline: Option<f64>) -> Result<Vec<u8>> {
    let mut bytes = vec![0; count];
    let mut read = 0;
    while read < count {
        let timeout = deadline.map(timeout_duration).transpose().map_err(|_| {
            Error::tuner(format!(
                "FireTuner response timed out after {read}/{count} bytes"
            ))
        })?;
        stream
            .set_read_timeout(timeout)
            .map_err(|e| Error::tuner(e.to_string()))?;
        match stream.read(&mut bytes[read..]) {
            Ok(0) => return Err(Error::tuner("FireTuner closed the connection")),
            Ok(received) => read += received,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
                ) =>
            {
                return Err(Error::tuner(format!(
                    "FireTuner response timed out after {read}/{count} bytes"
                )));
            }
            Err(error) => return Err(Error::tuner(error.to_string())),
        }
    }
    Ok(bytes)
}

/// Read one frame. `deadline` is an absolute value from [`monotonic`].
pub fn recv_frame(stream: &mut TcpStream, deadline: Option<f64>) -> Result<Frame> {
    let header = read_exact(stream, 8, deadline)?;
    let size = u32::from_le_bytes(header[0..4].try_into().expect("fixed frame header")) as usize;
    let tag = i32::from_le_bytes(header[4..8].try_into().expect("fixed frame header"));
    if size == 0 {
        return Ok(Frame {
            tag,
            payload: String::new(),
        });
    }
    if size > MAX_FRAME {
        return Err(Error::tuner(format!(
            "Invalid FireTuner frame size: {size}"
        )));
    }
    let body = read_exact(stream, size, deadline)?;
    let end = body
        .iter()
        .rposition(|&byte| byte != 0)
        .map_or(0, |index| index + 1);
    Ok(Frame {
        tag,
        payload: String::from_utf8_lossy(&body[..end]).into_owned(),
    })
}

fn checked_deadline(timeout: f64) -> Result<f64> {
    if !timeout.is_finite() || timeout <= 0.0 {
        return Err(Error::input("Invalid port or timeout"));
    }
    Ok(monotonic() + timeout)
}

fn state_pairs(raw: &str) -> Result<Vec<(i64, String)>> {
    let raw = raw.strip_prefix("LSQ:").unwrap_or(raw);
    let parts: Vec<&str> = raw
        .split(['\r', '\n', '\0'])
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect();
    let first_is_index = parts.first().is_some_and(|part| {
        let digits = part.strip_prefix('-').unwrap_or(part);
        !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit())
    });
    if parts.len() >= 2 && !first_is_index {
        return Ok(parts
            .into_iter()
            .enumerate()
            .map(|(index, name)| (index as i64, name.to_owned()))
            .collect());
    }
    let mut pairs = Vec::new();
    for chunk in parts.chunks_exact(2) {
        let index = chunk[0]
            .parse::<i64>()
            .map_err(|_| Error::tuner(format!("unexpected LSQ reply near {:?}", chunk)))?;
        pairs.push((index, chunk[1].to_owned()));
    }
    Ok(pairs)
}

pub struct TunerClient {
    pub port: u16,
    pub timeout: f64,
    pub identity: String,
    stream: Option<TcpStream>,
}

impl TunerClient {
    pub fn connect(port: u16, timeout: f64) -> Result<Self> {
        if port == 0 {
            return Err(Error::input("Invalid port or timeout"));
        }
        let deadline = checked_deadline(timeout)?;
        let address = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port));
        let stream = TcpStream::connect_timeout(&address, timeout_duration(deadline)?)
            .map_err(|e| Error::tuner(format!("Cannot connect to local FireTuner: {e}")))?;
        let mut client = Self {
            port,
            timeout,
            identity: String::new(),
            stream: Some(stream),
        };
        let setup = (|| {
            client.identity = client.request("APP:", deadline)?;
            if client.list_state_pairs_deadline(deadline)?.is_empty() {
                return Err(Error::tuner("FireTuner returned no Lua states"));
            }
            Ok(())
        })();
        if let Err(error) = setup {
            client.close();
            return Err(Error::tuner(format!(
                "Cannot connect to local FireTuner: {error}"
            )));
        }
        Ok(client)
    }

    pub fn close(&mut self) {
        if let Some(stream) = self.stream.take() {
            let _ = stream.shutdown(Shutdown::Both);
        }
    }

    fn socket(&mut self) -> Result<&mut TcpStream> {
        self.stream
            .as_mut()
            .ok_or_else(|| Error::tuner("FireTuner is not connected"))
    }

    fn request(&mut self, payload: &str, deadline: f64) -> Result<String> {
        let bytes = frame_bytes(HANDSHAKE, payload)?;
        if timeout_duration(deadline).is_err() {
            return Err(Error::tuner(format!(
                "No time left for {payload:?} FireTuner query"
            )));
        }
        write_bytes(self.socket()?, &bytes, Some(deadline))?;
        loop {
            if timeout_duration(deadline).is_err() {
                return Err(Error::tuner(format!(
                    "No reply to {payload:?} from FireTuner"
                )));
            }
            let frame = recv_frame(self.socket()?, Some(deadline))?;
            if frame.payload.is_empty()
                || frame.payload.starts_with("O ")
                || frame.payload.starts_with("O\0")
            {
                continue;
            }
            return Ok(frame.payload);
        }
    }

    fn list_state_pairs_deadline(&mut self, deadline: f64) -> Result<Vec<(i64, String)>> {
        state_pairs(&self.request("LSQ:", deadline)?)
    }

    pub fn list_state_pairs(&mut self, timeout: f64) -> Result<Vec<(i64, String)>> {
        self.list_state_pairs_deadline(checked_deadline(timeout)?)
    }

    pub fn list_states(&mut self, timeout: f64) -> Result<Vec<String>> {
        Ok(self
            .list_state_pairs(timeout)?
            .into_iter()
            .map(|(_, name)| name)
            .collect())
    }

    pub fn state_index(&mut self, name: &str, timeout: f64) -> Result<i64> {
        self.state_index_deadline(name, checked_deadline(timeout)?)
    }

    fn state_index_deadline(&mut self, name: &str, deadline: f64) -> Result<i64> {
        let pairs = self.list_state_pairs_deadline(deadline)?;
        let matches: Vec<i64> = pairs
            .iter()
            .filter(|(_, state)| state_name_eq(state, name))
            .map(|(index, _)| *index)
            .collect();
        if matches.len() != 1 {
            let names: Vec<&str> = pairs.iter().map(|(_, state)| state.as_str()).collect();
            return Err(Error::tuner(format!(
                "Lua state {name:?} not uniquely available; states: {names:?}"
            )));
        }
        Ok(matches[0])
    }

    fn send_lua_once(
        &mut self,
        state: &str,
        index: i64,
        code: &str,
        token: &str,
        deadline: f64,
    ) -> Result<()> {
        let bytes = frame_bytes(COMMAND, &lua_payload(index, code, token))?;
        if timeout_duration(deadline).is_err() {
            return Err(Error::tuner(format!(
                "No time left to send Lua command to {state:?}"
            )));
        }
        // From this point onward even a failed write can mean partial execution.
        if let Err(error) = write_bytes(self.socket()?, &bytes, Some(deadline)) {
            self.close();
            return Err(Error::unknown(format!(
                "Lua command to {state:?} may have run: {error}"
            )));
        }
        Ok(())
    }

    pub fn run_lua(&mut self, state: &str, code: &str, timeout: f64) -> Result<LuaResult> {
        validate_lua_code(code)?;
        let deadline = checked_deadline(timeout)?;
        let index = self.state_index_deadline(state, deadline)?;
        let token = format!("{TOKEN_PREFIX}{}", uuid::Uuid::new_v4().simple());
        self.send_lua_once(state, index, code, &token, deadline)?;
        let mut lines = Vec::new();
        loop {
            if timeout_duration(deadline).is_err() {
                self.close();
                return Err(Error::unknown(format!(
                    "No Lua acknowledgment from {state:?}; command may have run. Inspect logs before retrying"
                )));
            }
            let frame = match recv_frame(self.socket()?, Some(deadline)) {
                Ok(frame) => frame,
                Err(error) => {
                    self.close();
                    return Err(Error::unknown(format!(
                        "No Lua acknowledgment from {state:?}; command may have run: {error}"
                    )));
                }
            };
            lines.push(frame.payload);
            for line in lines.last().expect("just appended").split(['\r', '\n']) {
                let Some((_, acknowledgment)) = line.split_once(&format!("{token}|")) else {
                    continue;
                };
                let Some((status, value)) = acknowledgment.split_once('|') else {
                    continue;
                };
                if status == "ERR" {
                    return Err(Error::lua(format!("Lua error in {state}: {value}")));
                }
                if status == "OK" {
                    return Ok(LuaResult {
                        value: value.to_owned(),
                        output: lines.join("\n"),
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ErrorKind;
    use std::net::TcpListener;

    #[test]
    fn failed_command_write_is_unknown_even_if_no_bytes_arrive() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let stream = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (_peer, _) = listener.accept().unwrap();
        stream.shutdown(Shutdown::Write).unwrap();
        let mut client = TunerClient {
            port: 1,
            timeout: 1.0,
            identity: String::new(),
            stream: Some(stream),
        };
        let error = client
            .send_lua_once("InGame", 17, "return true", "TOKEN", monotonic() + 1.0)
            .unwrap_err();
        assert_eq!(error.kind, ErrorKind::LuaResponseUnknown);
        assert!(error.message.contains("may have run"));
        assert!(client.stream.is_none());
    }
}
