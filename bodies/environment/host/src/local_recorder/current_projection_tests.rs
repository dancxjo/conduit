use super::*;

#[test]
fn fresh_membership_recovery_selects_boot_and_replayed_nonce_preserves_view() {
    for replay in [false, true] {
        let f = Fixture::new();
        f.collect(0, 1);
        f.projection().refresh(&f.binding, &f.history).unwrap();
        let before = fs::read(&f.output).unwrap();
        f.collect(1, 2);
        f.projection().refresh(&f.binding, &f.history).unwrap();
        let expected = fs::read(&f.output).unwrap();
        assert_ne!(before, expected);
        fs::write(&f.output, &before).unwrap();
        let binding_before = fs::read(&f.binding).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&binding_before).unwrap();
        let continuity = f.root.join("continuity.json");
        let mut file = fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(&continuity)
            .unwrap();
        serde_json::to_writer(&mut file, &serde_json::json!({
            "invitation":{"schema":conduit_body::INVITATION_SCHEMA,"claim":value["invitation"],"secret":vec![22;32],"rendezvous":null},
            "root_key":value["root_key"],"producer_host":value["receipt"]["producer_host"],"producer_boot":value["receipt"]["producer_boot"]
        })).unwrap();
        drop(file);
        let checkpoint = fs::read(f.root.join("canonical-body-checkpoint.json")).unwrap();
        let (evidence, manager): (
            conduit_body::BodyBiographyEvidence,
            conduit_body::AdmissionManager,
        ) = serde_json::from_slice(&checkpoint).unwrap();
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let peer = listener.local_addr().unwrap().to_string();
        listener.set_nonblocking(true).unwrap();
        let key = f.key.clone();
        let boot = f.boot.clone();
        let producer = value["receipt"]["producer_host"]
            .as_str()
            .unwrap()
            .to_owned();
        let server = std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + Duration::from_secs(8);
            let mut result = None;
            for step in 0..2 {
                let mut socket = loop {
                    match listener.accept() {
                        Ok((socket, _)) => break socket,
                        Err(error)
                            if error.kind() == std::io::ErrorKind::WouldBlock
                                && std::time::Instant::now() < deadline =>
                        {
                            std::thread::sleep(Duration::from_millis(10))
                        }
                        Err(error) => panic!("probe fixture accept: {error}"),
                    }
                };
                socket
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let mut header = Vec::new();
                while !header.ends_with(b"\r\n\r\n") {
                    let mut byte = [0];
                    socket.read_exact(&mut byte).unwrap();
                    header.push(byte[0]);
                    assert!(header.len() < 1024);
                }
                let header = std::str::from_utf8(&header).unwrap();
                if step == 0 {
                    assert!(header.starts_with("POST /body/join "));
                    let length: usize = header
                        .lines()
                        .find_map(|line| line.strip_prefix("Content-Length: "))
                        .unwrap()
                        .trim()
                        .parse()
                        .unwrap();
                    let mut body = vec![0; length];
                    socket.read_exact(&mut body).unwrap();
                    let request: recovery::Request = serde_json::from_slice(&body).unwrap();
                    let verified = recovery::verify_current(
                        &manager,
                        &evidence.membership,
                        &request,
                        &producer.clone().into(),
                        &boot.clone().into(),
                        &key.verifying_key().to_bytes(),
                    )
                    .unwrap();
                    let signature = key.sign(&verified.transcript()).to_bytes();
                    let mut receipt = verified.signed(signature);
                    if replay {
                        receipt.request_nonce[0] ^= 1;
                        receipt.signature = key.sign(&receipt.transcript()).to_bytes().to_vec();
                    }
                    result = Some(serde_json::to_vec(&receipt).unwrap());
                    socket.write_all(b"HTTP/1.0 202 Accepted\r\n\r\n{\"schema\":\"conduit.environment/body-join-queued@1\",\"ticket\":7}").unwrap();
                } else {
                    assert!(header.starts_with("GET /body/join/result?id=7 "));
                    socket.write_all(b"HTTP/1.0 200 OK\r\n\r\n").unwrap();
                    socket.write_all(&result.take().unwrap()).unwrap();
                }
            }
        });
        let _ = Projection {
            producer_boot: "current",
            output: &f.output,
        }
        .after_attempt(&f.binding, &f.history, Some(&continuity), &peer);
        server.join().unwrap();
        assert_eq!(fs::read(&f.binding).unwrap(), binding_before);
        assert_eq!(
            fs::read(&f.output).unwrap(),
            if replay { before } else { expected }
        );
    }
}
