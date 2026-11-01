// Actual canonical admission/checkpoint plus real localhost HTTP client recovery.
#[test]
fn lost_producer_ticket_recovers_current_membership_without_readmission() {
    recover_lost_ticket(false);
}
#[test]
fn foreign_body_recovery_receipt_preserves_private_query_without_binding() {
    recover_lost_ticket(true);
}
fn recover_lost_ticket(foreign_body: bool) {
    use ed25519_dalek::Signer;
    use std::io::{BufRead, Read, Write};
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
    let directory = std::env::temp_dir().join(format!(
        "conduit-current-observer-{}-{}-{}",
        std::process::id(),
        foreign_body,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&directory)
        .unwrap();
    let (key, receipt, claim) = canonical_admission(&directory);
    let checkpoint = std::fs::read(directory.join("canonical-body-checkpoint.json")).unwrap();
    let (evidence, mut manager): (
        conduit_body::BodyBiographyEvidence,
        conduit_body::AdmissionManager,
    ) = serde_json::from_slice(&checkpoint).unwrap();
    manager.retire_unfinished_challenges();
    let retained = serde_json::to_vec(&manager).unwrap();
    let write = |path: &Path, value: &serde_json::Value| {
        let mut file = std::fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(path)
            .unwrap();
        serde_json::to_writer(&mut file, value).unwrap();
        file.sync_all().unwrap();
        std::fs::File::open(path.parent().unwrap())
            .unwrap()
            .sync_all()
            .unwrap();
    };
    let input = directory.join("bootstrap.json");
    write(
        &input,
        &serde_json::json!({
            "invitation":{"schema":conduit_body::INVITATION_SCHEMA,"claim":claim,"secret":vec![22;32],"rendezvous":null},
            "root_key":key.verifying_key().to_bytes(),"producer_host":receipt.producer_host,"producer_boot":receipt.producer_boot,
        }),
    );
    let state = directory.join("observer");
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&state)
        .unwrap();
    write(
        &state.join("pending-ticket.json"),
        &serde_json::json!({
            "ticket":7,"local_host":receipt.credential.host_id,"local_boot":receipt.credential.boot_id,
        }),
    );
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let peer = listener.local_addr().unwrap().to_string();
    listener.set_nonblocking(true).unwrap();
    let captured_state = state.clone();
    let server = std::thread::spawn(move || {
        let mut result = None;
        let deadline = std::time::Instant::now() + Duration::from_secs(15);
        for step in 0..3 {
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(e)
                        if e.kind() == std::io::ErrorKind::WouldBlock
                            && std::time::Instant::now() < deadline =>
                    {
                        std::thread::sleep(Duration::from_millis(10))
                    }
                    Err(e) => panic!("recovery fixture accept: {e}"),
                }
            };
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut reader = std::io::BufReader::new(&mut stream);
            let mut first = String::new();
            reader.read_line(&mut first).unwrap();
            let mut length = 0;
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" {
                    break;
                }
                if let Some((name, value)) = line.split_once(':') {
                    if name.eq_ignore_ascii_case("Content-Length") {
                        length = value.trim().parse::<usize>().unwrap();
                    }
                }
            }
            assert!(length <= 1024);
            let mut body = vec![0; length];
            reader.read_exact(&mut body).unwrap();
            drop(reader);
            let (status, bytes) = match step {
                0 => {
                    assert!(first.starts_with("GET /body/join/result?id=7 "));
                    (404, vec![])
                }
                1 => {
                    assert!(first.starts_with("POST /body/join "));
                    assert!(captured_state.join("recovery-request.json").is_file());
                    let query: recovery::Request = serde_json::from_slice(&body).unwrap();
                    assert_eq!(query.boot_id, receipt.credential.boot_id);
                    let current = recovery::verify_current(
                        &manager,
                        &evidence.membership,
                        &query,
                        &receipt.producer_host,
                        &"producer/boot-after-power-cycle".into(),
                        &key.verifying_key().to_bytes(),
                    )
                    .unwrap();
                    let signature = key.sign(&current.transcript()).to_bytes();
                    let mut current = current.signed(signature);
                    if foreign_body {
                        current.credential.body_id =
                            serde_json::from_value(serde_json::json!("ef".repeat(32))).unwrap();
                        current.signature = key.sign(&current.transcript()).to_bytes().to_vec();
                    }
                    result = Some(serde_json::to_vec(&current).unwrap());
                    (
                        202,
                        br#"{"schema":"conduit.environment/body-join-queued@1","ticket":9}"#
                            .to_vec(),
                    )
                }
                _ => {
                    assert!(first.starts_with("GET /body/join/result?id=9 "));
                    (200, result.take().unwrap())
                }
            };
            write!(
                stream,
                "HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                bytes.len()
            )
            .unwrap();
            stream.write_all(&bytes).unwrap();
        }
        assert_eq!(serde_json::to_vec(&manager).unwrap(), retained);
    });
    let result = crate::body_join::join(&input, &state, &peer);
    server.join().unwrap();
    assert!(state.join("recovery-request.json").is_file());
    let binding = state.join("recorder-binding.json");
    if foreign_body {
        assert!(result.is_err());
        assert!(!binding.exists());
    } else {
        result.unwrap();
        admitted_collector(&binding).unwrap();
        let value: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&binding).unwrap()).unwrap();
        assert_eq!(value["schema"], "conduit.environment/joined-recorder@2");
        assert_eq!(
            value["receipt"]["producer_boot"],
            "producer/boot-after-power-cycle"
        );
        assert!(value.get("secret").is_none());
        crate::body_join::join(&input, &state, "127.0.0.1:1").unwrap();
    }
    assert_eq!(
        std::fs::read(directory.join("canonical-body-checkpoint.json")).unwrap(),
        checkpoint
    );
    std::fs::remove_dir_all(directory).unwrap();
}
