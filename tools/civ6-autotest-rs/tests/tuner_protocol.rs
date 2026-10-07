use civ6_autotest_rs::tuner::{
    lua_payload, recv_frame, send_frame, validate_lua_code, Frame, TunerClient, COMMAND, HANDSHAKE,
    MAX_FRAME, MAX_LUA_CODE_BYTES,
};
use civ6_autotest_rs::ErrorKind;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::Command as ProcessCommand;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

#[derive(Clone, Copy)]
enum LuaReply {
    Ok,
    Error,
    NoAck,
    Disconnect,
}

struct FakeTuner {
    port: u16,
    received: Arc<Mutex<Vec<Frame>>>,
    server: JoinHandle<()>,
}

impl FakeTuner {
    fn start(reply: LuaReply, states: &'static str, noisy_queries: bool) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind fake tuner");
        let port = listener.local_addr().unwrap().port();
        let received = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&received);
        let server = thread::spawn(move || {
            let (mut connection, _) = listener.accept().expect("accept fake tuner");
            while let Ok(frame) = recv_frame(&mut connection, None) {
                let payload = frame.payload.clone();
                log.lock().unwrap().push(frame);
                if payload == "APP:" || payload == "LSQ:" {
                    if noisy_queries {
                        send_split(&mut connection, HANDSHAKE, "");
                        send_split(&mut connection, COMMAND, "O unsolicited game output");
                    }
                    let answer = if payload == "APP:" {
                        "Civilization VI"
                    } else {
                        states
                    };
                    send_split(&mut connection, HANDSHAKE, answer);
                } else if payload.starts_with("CMD:") {
                    match reply {
                        LuaReply::Ok | LuaReply::Error => {
                            let token = extract_token(&payload);
                            send_split(&mut connection, COMMAND, "unsolicited output");
                            let answer = if matches!(reply, LuaReply::Error) {
                                format!("{token}|ERR|boom")
                            } else {
                                format!("{token}|OK|true")
                            };
                            send_split(&mut connection, COMMAND, &answer);
                        }
                        LuaReply::NoAck => thread::sleep(Duration::from_millis(350)),
                        LuaReply::Disconnect => return,
                    }
                }
            }
        });
        Self {
            port,
            received,
            server,
        }
    }

    fn finish(self) -> Vec<Frame> {
        self.server.join().expect("fake tuner thread");
        Arc::try_unwrap(self.received)
            .unwrap()
            .into_inner()
            .unwrap()
    }
}

fn extract_token(payload: &str) -> &str {
    let start = payload.find("JAC_AUTOTEST_").expect("ack token");
    let end = start + "JAC_AUTOTEST_".len() + 32;
    let token = &payload[start..end];
    assert!(token["JAC_AUTOTEST_".len()..]
        .bytes()
        .all(|byte| byte.is_ascii_hexdigit()));
    token
}

fn send_split(connection: &mut TcpStream, tag: i32, payload: &str) {
    let body = payload.as_bytes();
    let mut wire = Vec::new();
    wire.extend_from_slice(&((body.len() + 1) as u32).to_le_bytes());
    wire.extend_from_slice(&tag.to_le_bytes());
    wire.extend_from_slice(body);
    wire.push(0);
    connection.write_all(&wire[..3]).unwrap();
    connection.write_all(&wire[3..9]).unwrap();
    connection.write_all(&wire[9..]).unwrap();
}

fn tcp_pair() -> (TcpStream, TcpStream) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let left = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    let (right, _) = listener.accept().unwrap();
    (left, right)
}

const NAMED_STATES: &str = "3\0FrontEnd\017\0InGame\041\0GameCore_Tuner";

#[test]
fn japan_wrappers_receive_commands_at_the_observed_context_indices() {
    // Captured live04 indices: stock staging 17, wrapper staging 26, wrapper menu 27.
    const JAPAN_STATES: &str = "17\0StagingRoom\026\0JAC_JapanStagingRoom\027\0JAC_JapanMainMenu";
    let fake = FakeTuner::start(LuaReply::Ok, JAPAN_STATES, false);
    let mut client = TunerClient::connect(fake.port, 1.0).unwrap();
    client.run_lua("MainMenu", "return true", 1.0).unwrap();
    client.run_lua("stagingroom", "return true", 1.0).unwrap();
    client.close();
    let commands: Vec<_> = fake
        .finish()
        .into_iter()
        .filter(|f| f.tag == COMMAND)
        .collect();
    assert_eq!(commands.len(), 2);
    assert!(commands[0].payload.starts_with("CMD:27:"));
    assert!(commands[1].payload.starts_with("CMD:26:"));
}

#[test]
fn ambiguous_japan_wrappers_and_unregistered_suffixes_never_receive_commands() {
    for (states, requested) in [
        (
            "17\0StagingRoom\026\0JAC_JapanStagingRoom\027\0jac_japanstagingroom",
            "StagingRoom",
        ),
        (
            "1\0MainMenu\026\0JAC_JapanMainMenu\027\0jac_japanmainmenu",
            "MainMenu",
        ),
        ("17\0StagingRoom\026\0stagingroom", "StagingRoom"),
        ("1\0Other_MainMenu\02\0MainMenu_Custom", "MainMenu"),
    ] {
        let fake = FakeTuner::start(LuaReply::Ok, states, false);
        let mut client = TunerClient::connect(fake.port, 1.0).unwrap();
        assert_eq!(
            client
                .run_lua(requested, "return true", 1.0)
                .unwrap_err()
                .kind,
            ErrorKind::Tuner
        );
        client.close();
        assert_eq!(fake.finish().iter().filter(|f| f.tag == COMMAND).count(), 0);
    }
}

#[test]
fn round_trip_fragmented_and_named_state() {
    let fake = FakeTuner::start(LuaReply::Ok, NAMED_STATES, false);
    let mut client = TunerClient::connect(fake.port, 1.0).unwrap();
    assert_eq!(client.identity, "Civilization VI");
    assert_eq!(
        client.list_state_pairs(1.0).unwrap(),
        [
            (3, "FrontEnd".to_owned()),
            (17, "InGame".to_owned()),
            (41, "GameCore_Tuner".to_owned()),
        ]
    );
    assert_eq!(
        client.list_states(1.0).unwrap(),
        ["FrontEnd", "InGame", "GameCore_Tuner"]
    );
    let result = client.run_lua("InGame", "return true", 1.0).unwrap();
    assert_eq!(result.value, "true");
    assert!(result.output.contains("unsolicited output"));
    client.close();
    let commands: Vec<Frame> = fake
        .finish()
        .into_iter()
        .filter(|frame| frame.tag == COMMAND)
        .collect();
    assert_eq!(commands.len(), 1);
    assert!(commands[0].payload.starts_with("CMD:17:"));
    assert!(commands[0]
        .payload
        .contains("pcall(function()\nreturn true\nend)"));
}

#[test]
fn lua_error_is_reported_without_retry() {
    let fake = FakeTuner::start(LuaReply::Error, NAMED_STATES, false);
    let mut client = TunerClient::connect(fake.port, 1.0).unwrap();
    let error = client.run_lua("InGame", "error('boom')", 1.0).unwrap_err();
    assert_eq!(error.kind, ErrorKind::LuaExecution);
    assert!(error.message.contains("boom"));
    client.close();
    assert_eq!(
        fake.finish()
            .iter()
            .filter(|frame| frame.tag == COMMAND)
            .count(),
        1
    );
}

#[test]
fn missing_ack_is_unknown_and_not_replayed() {
    let fake = FakeTuner::start(LuaReply::NoAck, NAMED_STATES, false);
    let mut client = TunerClient::connect(fake.port, 0.15).unwrap();
    let error = client.run_lua("InGame", "return true", 0.1).unwrap_err();
    assert_eq!(error.kind, ErrorKind::LuaResponseUnknown);
    assert!(error.message.contains("may have run"));
    assert_eq!(client.list_states(0.1).unwrap_err().kind, ErrorKind::Tuner);
    assert_eq!(
        fake.finish()
            .iter()
            .filter(|frame| frame.tag == COMMAND)
            .count(),
        1
    );
}

#[test]
fn missing_state_fails_before_lua_send() {
    let fake = FakeTuner::start(LuaReply::Ok, NAMED_STATES, false);
    let mut client = TunerClient::connect(fake.port, 1.0).unwrap();
    let error = client.run_lua("Missing", "return true", 1.0).unwrap_err();
    assert_eq!(error.kind, ErrorKind::Tuner);
    assert!(error.message.contains("not uniquely available"));
    client.close();
    assert_eq!(
        fake.finish()
            .iter()
            .filter(|frame| frame.tag == COMMAND)
            .count(),
        0
    );
}

#[test]
fn connection_loss_after_send_is_unknown() {
    let fake = FakeTuner::start(LuaReply::Disconnect, NAMED_STATES, false);
    let mut client = TunerClient::connect(fake.port, 1.0).unwrap();
    let error = client.run_lua("InGame", "return true", 1.0).unwrap_err();
    assert_eq!(error.kind, ErrorKind::LuaResponseUnknown);
    assert!(error.message.contains("may have run"));
    assert_eq!(
        fake.finish()
            .iter()
            .filter(|frame| frame.tag == COMMAND)
            .count(),
        1
    );
}

#[test]
fn rejects_oversized_frame() {
    let (mut left, mut right) = tcp_pair();
    right
        .write_all(&((MAX_FRAME + 1) as u32).to_le_bytes())
        .unwrap();
    right.write_all(&COMMAND.to_le_bytes()).unwrap();
    let error = recv_frame(&mut left, None).unwrap_err();
    assert_eq!(error.kind, ErrorKind::Tuner);
    assert!(error.message.contains("size"));
}

#[test]
fn empty_frame_is_not_fatal() {
    let (mut left, mut right) = tcp_pair();
    right.write_all(&0_u32.to_le_bytes()).unwrap();
    right.write_all(&COMMAND.to_le_bytes()).unwrap();
    assert_eq!(
        recv_frame(&mut left, None).unwrap(),
        Frame {
            tag: COMMAND,
            payload: String::new()
        }
    );
}

#[test]
fn send_nul_is_rejected_before_network_write() {
    let (mut left, mut right) = tcp_pair();
    let error = send_frame(&mut left, COMMAND, "bad\0command").unwrap_err();
    assert_eq!(error.kind, ErrorKind::Input);
    right
        .set_read_timeout(Some(Duration::from_millis(20)))
        .unwrap();
    let mut byte = [0];
    assert!(right.read(&mut byte).is_err());
}

#[test]
fn query_ignores_game_output_and_empty_frames() {
    let fake = FakeTuner::start(LuaReply::Ok, NAMED_STATES, true);
    let mut client = TunerClient::connect(fake.port, 1.0).unwrap();
    assert_eq!(client.identity, "Civilization VI");
    assert_eq!(
        client.list_states(1.0).unwrap(),
        ["FrontEnd", "InGame", "GameCore_Tuner"]
    );
    client.close();
    let received = fake.finish();
    assert_eq!(
        received
            .iter()
            .filter(|frame| frame.payload == "APP:")
            .count(),
        1
    );
}

#[test]
fn names_only_reply_uses_positions_and_case_insensitive_lookup() {
    let fake = FakeTuner::start(LuaReply::Ok, "LSQ:FrontEnd\nInGame\nGameCore_Tuner", false);
    let mut client = TunerClient::connect(fake.port, 1.0).unwrap();
    assert_eq!(
        client.list_state_pairs(1.0).unwrap()[1],
        (1, "InGame".to_owned())
    );
    client.run_lua("ingame", "return true", 1.0).unwrap();
    client.close();
    let commands: Vec<_> = fake
        .finish()
        .into_iter()
        .filter(|frame| frame.tag == COMMAND)
        .collect();
    assert_eq!(commands.len(), 1);
    assert!(commands[0].payload.starts_with("CMD:1:"));
}

#[test]
fn duplicate_state_name_is_rejected_before_send() {
    let fake = FakeTuner::start(LuaReply::Ok, "0\0InGame\01\0ingame", false);
    let mut client = TunerClient::connect(fake.port, 1.0).unwrap();
    let error = client.run_lua("InGame", "return true", 1.0).unwrap_err();
    assert_eq!(error.kind, ErrorKind::Tuner);
    assert!(error.message.contains("not uniquely available"));
    client.close();
    assert_eq!(
        fake.finish()
            .iter()
            .filter(|frame| frame.tag == COMMAND)
            .count(),
        0
    );
}

#[test]
fn unicode_state_names_use_full_default_case_folding() {
    let fake = FakeTuner::start(LuaReply::Ok, "17\0Straße\n18\nﬂ", false);
    let mut client = TunerClient::connect(fake.port, 1.0).unwrap();
    assert_eq!(client.state_index("STRASSE", 1.0).unwrap(), 17);
    assert_eq!(client.state_index("FL", 1.0).unwrap(), 18);
    client.run_lua("STRASSE", "return true", 1.0).unwrap();
    client.close();
    let commands: Vec<_> = fake
        .finish()
        .into_iter()
        .filter(|frame| frame.tag == COMMAND)
        .collect();
    assert_eq!(commands.len(), 1);
    assert!(commands[0].payload.starts_with("CMD:17:"));
}

#[test]
fn invalid_integer_ports_are_parsed_before_tuner_runtime_validation() {
    for port in [
        "0",
        "-1",
        "65536",
        "18446744073709551616",
        "-18446744073709551616",
    ] {
        let output = ProcessCommand::new(env!("CARGO_BIN_EXE_civ6-tuner"))
            .args(["states", "--port", port])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(String::from_utf8_lossy(&output.stderr).contains("Invalid port or timeout"));
    }
}

#[test]
fn lua_code_limit_counts_utf8_bytes_and_wrapper() {
    assert_eq!(lua_payload(17, "return true", "TOKEN"),
        "CMD:17:local __ok, __value = pcall(function()\nreturn true\nend)\nprint(\"TOKEN|\" .. (__ok and \"OK|\" or \"ERR|\") .. tostring(__value))");
    let at_limit = "a".repeat(MAX_LUA_CODE_BYTES);
    validate_lua_code(&at_limit).unwrap();
    assert_eq!(
        lua_payload(
            i64::MIN,
            &at_limit,
            &format!("JAC_AUTOTEST_{}", "0".repeat(32))
        )
        .len()
            + 1,
        MAX_FRAME
    );
    assert_eq!(
        validate_lua_code(&(at_limit + "a")).unwrap_err().kind,
        ErrorKind::Input
    );
    assert_eq!(
        validate_lua_code(&"界".repeat(MAX_LUA_CODE_BYTES / 3 + 1))
            .unwrap_err()
            .kind,
        ErrorKind::Input
    );
    assert_eq!(
        validate_lua_code("return '\0'").unwrap_err().kind,
        ErrorKind::Input
    );
}

#[test]
fn invalid_connect_arguments_are_rejected() {
    for timeout in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        assert_eq!(
            TunerClient::connect(4318, timeout).err().unwrap().kind,
            ErrorKind::Input
        );
    }
    assert_eq!(
        TunerClient::connect(0, 1.0).err().unwrap().kind,
        ErrorKind::Input
    );
}

#[test]
fn states_command_prints_identity_and_real_state_indices() {
    let fake = FakeTuner::start(LuaReply::Ok, NAMED_STATES, false);
    let output = ProcessCommand::new(env!("CARGO_BIN_EXE_civ6-tuner"))
        .args(["states", "--port", &fake.port.to_string()])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "Civilization VI\n3: FrontEnd\n17: InGame\n41: GameCore_Tuner\n"
    );
    assert_eq!(
        fake.finish()
            .iter()
            .filter(|frame| frame.payload == "LSQ:")
            .count(),
        2
    );
}

#[test]
fn lua_command_prints_acknowledged_output() {
    let fake = FakeTuner::start(LuaReply::Ok, NAMED_STATES, false);
    let output = ProcessCommand::new(env!("CARGO_BIN_EXE_civ6-tuner"))
        .args([
            "lua",
            "--state",
            "InGame",
            "--code",
            "return true",
            "--port",
            &fake.port.to_string(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("unsolicited output"));
    assert!(stdout.contains("|OK|true"));
    assert_eq!(
        fake.finish()
            .iter()
            .filter(|frame| frame.tag == COMMAND)
            .count(),
        1
    );
}

#[test]
fn lua_command_requires_code_before_connecting() {
    let output = ProcessCommand::new(env!("CARGO_BIN_EXE_civ6-tuner"))
        .arg("lua")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("lua requires --code"));
}
