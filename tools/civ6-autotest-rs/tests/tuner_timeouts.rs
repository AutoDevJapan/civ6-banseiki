use civ6_autotest_rs::time::monotonic;
use civ6_autotest_rs::tuner::{recv_frame, send_frame, TunerClient, COMMAND, HANDSHAKE};
use civ6_autotest_rs::ErrorKind;
use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

fn accept_pair() -> (TcpStream, TcpStream) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    let (server, _) = listener.accept().unwrap();
    (client, server)
}

fn answer_setup(connection: &mut TcpStream) {
    assert_eq!(recv_frame(connection, None).unwrap().payload, "APP:");
    send_frame(connection, HANDSHAKE, "Civilization VI").unwrap();
    assert_eq!(recv_frame(connection, None).unwrap().payload, "LSQ:");
    send_frame(connection, HANDSHAKE, "0\nInGame").unwrap();
}

#[test]
fn partial_header_timeout_reports_received_byte_count() {
    let (mut client, mut server) = accept_pair();
    let writer = thread::spawn(move || {
        server.write_all(&[5, 0, 0]).unwrap();
        thread::sleep(Duration::from_millis(120));
    });
    let error = recv_frame(&mut client, Some(monotonic() + 0.05)).unwrap_err();
    assert_eq!(error.kind, ErrorKind::Tuner);
    assert!(error.message.contains("timed out after 3/8 bytes"));
    writer.join().unwrap();
}

#[test]
fn partial_body_timeout_reports_received_byte_count() {
    let (mut client, mut server) = accept_pair();
    let writer = thread::spawn(move || {
        server.write_all(&5_u32.to_le_bytes()).unwrap();
        server.write_all(&COMMAND.to_le_bytes()).unwrap();
        server.write_all(b"ab").unwrap();
        thread::sleep(Duration::from_millis(120));
    });
    let error = recv_frame(&mut client, Some(monotonic() + 0.05)).unwrap_err();
    assert_eq!(error.kind, ErrorKind::Tuner);
    assert!(error.message.contains("timed out after 2/5 bytes"));
    writer.join().unwrap();
}

#[test]
fn timed_out_query_can_use_a_fresh_connection() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let requests = Arc::new(Mutex::new(Vec::new()));
    let seen = Arc::clone(&requests);
    let server = thread::spawn(move || {
        for attempt in 0..2 {
            let (mut connection, _) = listener.accept().unwrap();
            answer_setup(&mut connection);
            let query = recv_frame(&mut connection, None).unwrap();
            seen.lock().unwrap().push((attempt, query.payload));
            if attempt == 0 {
                thread::sleep(Duration::from_millis(130));
            } else {
                send_frame(&mut connection, HANDSHAKE, "0\nInGame").unwrap();
            }
        }
    });

    let mut first = TunerClient::connect(port, 1.0).unwrap();
    let error = first.list_states(0.05).unwrap_err();
    assert_eq!(error.kind, ErrorKind::Tuner);
    first.close();
    let mut second = TunerClient::connect(port, 1.0).unwrap();
    assert_eq!(second.list_states(1.0).unwrap(), ["InGame"]);
    second.close();
    server.join().unwrap();
    assert_eq!(
        *requests.lock().unwrap(),
        [(0, "LSQ:".to_owned()), (1, "LSQ:".to_owned())]
    );
}

#[test]
fn asynchronous_output_does_not_extend_lua_ack_deadline_or_replay_mutation() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let commands = Arc::new(Mutex::new(Vec::new()));
    let received = Arc::clone(&commands);
    let server = thread::spawn(move || {
        let (mut connection, _) = listener.accept().unwrap();
        answer_setup(&mut connection);
        assert_eq!(recv_frame(&mut connection, None).unwrap().payload, "LSQ:");
        send_frame(&mut connection, HANDSHAKE, "0\nInGame").unwrap();
        let command = recv_frame(&mut connection, None).unwrap();
        assert_eq!(command.tag, COMMAND);
        received.lock().unwrap().push(command.payload);
        for _ in 0..40 {
            thread::sleep(Duration::from_millis(20));
            if send_frame(&mut connection, COMMAND, "O still running").is_err() {
                break;
            }
        }
    });
    let mut client = TunerClient::connect(port, 1.0).unwrap();
    let started = Instant::now();
    let error = client
        .run_lua("InGame", "AutoplayManager.SetActive(true)", 0.08)
        .unwrap_err();
    assert_eq!(error.kind, ErrorKind::LuaResponseUnknown);
    assert!(error.message.contains("may have run"));
    assert!(started.elapsed() < Duration::from_millis(600));
    assert_eq!(client.list_states(0.1).unwrap_err().kind, ErrorKind::Tuner);
    server.join().unwrap();
    let commands = commands.lock().unwrap();
    assert_eq!(commands.len(), 1);
    assert!(commands[0].contains("AutoplayManager.SetActive(true)"));
}
