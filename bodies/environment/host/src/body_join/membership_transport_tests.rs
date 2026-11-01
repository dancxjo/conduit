use super::*;
use std::{
    net::{TcpListener, TcpStream},
    os::unix::fs::DirBuilderExt,
    path::PathBuf,
};

fn fixture() -> (PathBuf, PathBuf, Request) {
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "conduit-membership-transport-{}-{unique}",
        std::process::id()
    ));
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&root)
        .unwrap();
    let request: Request = serde_json::from_value(serde_json::json!({
        "schema":conduit_body::MEMBERSHIP_ADMISSION_REQUEST_SCHEMA,
        "protocol_version":conduit_core::PROTOCOL_VERSION,
        "invitation_id":"01".repeat(32), "body_id":"02".repeat(32),
        "host_id":format!("host/installed/{}", "03".repeat(32)),
        "boot_id":format!("boot/installed/{}", "04".repeat(32)),
        "offer_generation":2,"nonce":vec![255;32],"signature":vec![255;64],
    }))
    .unwrap();
    let input = root.join("request.json");
    save(&input, &request).unwrap();
    (root, input, request)
}
fn candidate(request: &Request) -> Receipt {
    serde_json::from_value(serde_json::json!({
        "schema":conduit_body::MEMBERSHIP_ADMISSION_RECEIPT_SCHEMA,
        "protocol_version":conduit_core::PROTOCOL_VERSION,
        "credential":{"credential_id":"05".repeat(32),"body_id":request.body_id,
            "part_id":"06".repeat(32),"host_id":request.host_id,"boot_id":request.boot_id,
            "issued_at_millis":1},
        "membership_revision":1,"request_digest":request.digest(),
        "producer_host":"host/root","producer_boot":"boot/root","producer_key":vec![0;32],
        "invitation_id":request.invitation_id,"offer_generation":request.offer_generation,
        "membership_admitted":true,"current_offers_available":false,"plan_created":false,
        "play_created":false,"signature":vec![0;64],
    }))
    .unwrap()
}
fn accept(listener: &TcpListener) -> TcpStream {
    listener.set_nonblocking(true).unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        match listener.accept() {
            Ok((stream, _)) => {
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                return stream;
            }
            Err(error)
                if error.kind() == std::io::ErrorKind::WouldBlock && Instant::now() < deadline =>
            {
                std::thread::sleep(Duration::from_millis(2))
            }
            Err(error) => panic!("bounded membership fixture accept: {error}"),
        }
    }
}
fn read(stream: &mut TcpStream) -> (String, Vec<u8>) {
    let mut wire = Vec::new();
    loop {
        let mut byte = [0];
        stream.read_exact(&mut byte).unwrap();
        wire.push(byte[0]);
        assert!(wire.len() <= 1024);
        if wire.ends_with(b"\r\n\r\n") {
            break;
        }
    }
    let header = String::from_utf8(wire.clone()).unwrap();
    let length: usize = header
        .lines()
        .find_map(|line| line.strip_prefix("Content-Length: "))
        .unwrap()
        .parse()
        .unwrap();
    assert!(
        wire.len() + length <= 1024,
        "real installed identity frame exceeds native HTTP bound"
    );
    let mut body = vec![0; length];
    stream.read_exact(&mut body).unwrap();
    (header, body)
}
fn reply(stream: &mut TcpStream, status: u16, bytes: &[u8]) {
    write!(
        stream,
        "HTTP/1.1 {status} OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        bytes.len()
    )
    .unwrap();
    stream.write_all(bytes).unwrap();
}
fn server(listener: TcpListener, request: Request) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut post = accept(&listener);
        let (header, body) = read(&mut post);
        assert!(header.starts_with("POST /body/join HTTP/1.1\r\n"));
        assert_eq!(Request::decode(&body).unwrap(), request);
        reply(
            &mut post,
            202,
            br#"{"schema":"conduit.environment/body-join-queued@1","ticket":7}"#,
        );
        drop(post);
        let mut get = accept(&listener);
        let (header, empty) = read(&mut get);
        assert!(header.starts_with("GET /body/join/result?id=7 HTTP/1.1\r\n"));
        assert!(empty.is_empty());
        reply(
            &mut get,
            200,
            &serde_json::to_vec(&candidate(&request)).unwrap(),
        );
        body
    })
}

#[test]
fn existing_http_transports_bounded_installed_identity_without_granting_membership() {
    let (root, input, request) = fixture();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let peer = listener.local_addr().unwrap().to_string();
    let worker = server(listener, request.clone());
    let state = root.join("transport");
    run(&input, &state, &peer).unwrap();
    worker.join().unwrap();
    let retained: Receipt = load(&state.join("membership-candidate-receipt.json")).unwrap();
    retained.validate_against(&request).unwrap();
    assert!(
        retained.verify(&[7; 32], &request).is_err(),
        "transport correlation is not Root authentication"
    );
    assert!(!state.join("recorder-binding.json").exists());
    assert!(!state.join("installation.json").exists());
    assert!(!state.join("continuity-private.json").exists());
    run(&input, &state, &peer).unwrap(); // Reopen the same candidate without network.
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn lost_post_response_replays_only_original_proof_and_preserves_attempt_identity() {
    let (root, input, request) = fixture();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let peer = listener.local_addr().unwrap().to_string();
    let first = std::thread::spawn(move || {
        let mut post = accept(&listener);
        read(&mut post).1
    });
    let state = root.join("transport");
    assert!(run(&input, &state, &peer).is_err());
    let original = first.join().unwrap();
    assert!(!state.join("pending-ticket.json").exists());
    assert_eq!(
        self::request(&state.join("submission-private.json")).unwrap(),
        request
    );
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let peer = listener.local_addr().unwrap().to_string();
    let second = server(listener, request.clone());
    run(&input, &state, &peer).unwrap();
    assert_eq!(
        second.join().unwrap(),
        original,
        "retry changed the wire proof"
    );
    let mut different = request;
    different.boot_id = "boot/another".into();
    let changed = root.join("changed.json");
    save(&changed, &different).unwrap();
    assert!(run(&changed, &state, &peer).is_err());
    std::fs::remove_dir_all(root).unwrap();
}
