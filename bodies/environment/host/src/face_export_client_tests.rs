use super::*;
use ed25519_dalek::{Signer, SigningKey};
fn offered(
    key: &SigningKey,
    body: &conduit_body::BodyId,
    nonce: [u8; 32],
    lifetime: u64,
) -> Vec<u8> {
    let boot = "boot/face-producer";
    let mut payload = [0; 156];
    payload[..32].copy_from_slice(&nonce);
    payload[32..64].copy_from_slice(&Sha256::digest(body.as_str().as_bytes()));
    payload[64..96].copy_from_slice(&Sha256::digest(boot.as_bytes()));
    payload[96..128].fill(9);
    payload[128..136].copy_from_slice(&7u64.to_le_bytes());
    payload[136..140].copy_from_slice(&1024u32.to_le_bytes());
    payload[140..148].copy_from_slice(&100u64.to_le_bytes());
    payload[148..156].copy_from_slice(&(100 + lifetime).to_le_bytes());
    let mut hash = Sha256::new();
    hash.update(b"conduit.face/export@1\0");
    hash.update(key.verifying_key().to_bytes());
    hash.update(payload);
    let mut signed = payload.to_vec();
    signed.extend_from_slice(&key.sign(&hash.finalize()).to_bytes());
    serde_json::to_vec(&serde_json::json!({"profile":"conduit.face/export@1","producer_boot":boot,"body_id":body,"offer":hex(&signed)})).unwrap()
}
#[test]
fn root_signature_nonce_body_and_finite_lifetime_are_all_required() {
    let key = SigningKey::from_bytes(&[17; 32]);
    let body: conduit_body::BodyId =
        serde_json::from_str(&format!("\"{}\"", "a".repeat(64))).unwrap();
    let nonce = [4; 32];
    let root = key.verifying_key().to_bytes();
    let reply = offered(&key, &body, nonce, 5000);
    let verified = verify(&root, &body, &nonce, &reply).unwrap();
    assert_eq!(
        (verified.bytes(), verified.revision(), verified.lifetime()),
        (1024, 7, 5000)
    );
    assert!(verify(&root, &body, &[5; 32], &reply).is_err());
    assert!(verify(
        &SigningKey::from_bytes(&[18; 32]).verifying_key().to_bytes(),
        &body,
        &nonce,
        &reply
    )
    .is_err());
    assert!(verify(
        &root,
        &serde_json::from_str(&format!("\"{}\"", "b".repeat(64))).unwrap(),
        &nonce,
        &reply
    )
    .is_err());
    for lifetime in [0, 5001] {
        assert!(verify(&root, &body, &nonce, &offered(&key, &body, nonce, lifetime)).is_err());
    }
    let mut value: serde_json::Value = serde_json::from_slice(&reply).unwrap();
    value["producer_boot"] = "boot/foreign".into();
    assert!(verify(&root, &body, &nonce, &serde_json::to_vec(&value).unwrap()).is_err());
}
#[test]
fn atomic_local_envelope_preserves_full_face_and_cannot_extend_expired_lease() {
    use std::os::unix::fs::DirBuilderExt;
    let directory = std::env::temp_dir().join(format!(
        "conduit-face-publisher-{}-{}",
        std::process::id(),
        boot_ms().unwrap()
    ));
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&directory)
        .unwrap();
    let path = directory.join("current-face.json");
    let face = conduit_face::Face::new(
        7,
        conduit_face::FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id: None,
            checked_plot_id: None,
            expanded_plot_id: None,
            plan_id: None,
            active_play_id: None,
            sign_ids: vec![],
        },
        vec![conduit_face::FaceSubject {
            identity: "environment".into(),
            role: conduit_face::FaceRole::Info,
            name: "Fixture".into(),
        }],
        vec![],
        vec![],
        vec![],
    )
    .unwrap();
    let deadline = boot_ms().unwrap() + 3000;
    publish(&path, &face, deadline).unwrap();
    let bytes = std::fs::read(&path).unwrap();
    let envelope: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(envelope["valid_until_boot_ms"], deadline);
    assert_eq!(
        envelope["boot_id"],
        std::fs::read_to_string("/proc/sys/kernel/random/boot_id")
            .unwrap()
            .trim()
    );
    let decoded: conduit_face::Face = serde_json::from_value(envelope["face"].clone()).unwrap();
    assert_eq!(decoded, face);
    decoded.validate().unwrap();
    assert!(publish(&path, &face, 0).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
    assert_eq!(std::fs::metadata(&path).unwrap().mode() & 0o777, 0o600);
    assert_eq!(std::fs::read_dir(&directory).unwrap().count(), 1);
    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn transient_transport_and_busy_replies_share_the_original_absolute_deadline() {
    use std::cell::Cell;
    use std::io::{Error, ErrorKind};
    let attempts = Cell::new(0);
    let elapsed = Cell::new(0);
    let result = get_until(
        || {
            attempts.set(attempts.get() + 1);
            match attempts.get() {
                1 => Err(Error::from(ErrorKind::ConnectionRefused).into()),
                2 => Ok((503, vec![])),
                3 => Ok((202, vec![])),
                _ => Ok((200, vec![7])),
            }
        },
        || Ok(100 + elapsed.get()),
        || elapsed.set(elapsed.get() + 50),
        100,
    )
    .unwrap();
    assert_eq!(result, vec![7]);
    assert_eq!(attempts.get(), 4);
    attempts.set(0);
    let result = get_until(
        || {
            attempts.set(attempts.get() + 1);
            Ok((202, vec![]))
        },
        || {
            let n = elapsed.get();
            elapsed.set(n + 2500);
            Ok(100 + n)
        },
        || {},
        100,
    );
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("absolute deadline"));
    assert_eq!(attempts.get(), 2);
    for status in [400, 403, 409] {
        attempts.set(0);
        assert!(get_until(
            || {
                attempts.set(attempts.get() + 1);
                Ok((status, vec![]))
            },
            || Ok(100),
            || panic!("permanent refusal retried"),
            100
        )
        .is_err());
        assert_eq!(attempts.get(), 1);
    }
    assert!(get_until(
        || Err(Error::from(ErrorKind::InvalidData).into()),
        || Ok(100),
        || panic!("malformed reply retried"),
        100
    )
    .is_err());
}

#[test]
fn pending_export_can_use_its_full_budget_without_abandoning_its_nonce() {
    use std::cell::Cell;
    let elapsed = Cell::new(0);
    let attempts = Cell::new(0);
    let bytes = get_until(
        || {
            attempts.set(attempts.get() + 1);
            Ok(if elapsed.get() < 3000 {
                (202, vec![])
            } else {
                (200, vec![9])
            })
        },
        || Ok(100 + elapsed.get()),
        || elapsed.set(elapsed.get() + RETRY_PAUSE_MS),
        100,
    )
    .unwrap();
    assert_eq!(bytes, vec![9]);
    assert_eq!(attempts.get(), 61);

    elapsed.set(0);
    attempts.set(0);
    let error = get_until(
        || {
            attempts.set(attempts.get() + 1);
            Ok((503, vec![]))
        },
        || Ok(100 + elapsed.get()),
        || elapsed.set(elapsed.get() + RETRY_PAUSE_MS),
        100,
    )
    .unwrap_err();
    assert!(error.to_string().contains("absolute deadline"));
    assert_eq!(elapsed.get(), FETCH_BUDGET_MS);
    assert_eq!(attempts.get(), FETCH_BUDGET_MS / RETRY_PAUSE_MS);

    // A stalled clock still cannot turn this bounded reader into an unbounded
    // request loop.
    attempts.set(0);
    let error = get_until(
        || {
            attempts.set(attempts.get() + 1);
            Ok((202, vec![]))
        },
        || Ok(100),
        || {},
        100,
    )
    .unwrap_err();
    assert!(error.to_string().contains("polling bound"));
    assert_eq!(attempts.get(), MAXIMUM_ATTEMPTS);
}

#[test]
fn refresh_cadence_includes_transfer_time_without_extending_the_lease() {
    assert_eq!(
        super::refresh_delay(100, 100),
        std::time::Duration::from_millis(1000)
    );
    assert_eq!(
        super::refresh_delay(100, 850),
        std::time::Duration::from_millis(250)
    );
    assert_eq!(super::refresh_delay(100, 2100), std::time::Duration::ZERO);
    assert_eq!(super::refresh_delay(100, 5100), std::time::Duration::ZERO);
}

#[test]
fn a_slow_socket_cannot_restart_the_remaining_fetch_budget() {
    use std::io::{Read, Write};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut request = Vec::new();
        while !request.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            socket.read_exact(&mut byte).unwrap();
            request.push(byte[0]);
            assert!(request.len() <= 1024);
        }
        std::thread::sleep(Duration::from_millis(250));
        // The client may already have closed its original bounded exchange.
        let _ = socket
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}");
    });
    let started = super::boot_ms()
        .unwrap()
        .checked_sub(FETCH_BUDGET_MS - 100)
        .unwrap();
    let result = super::get(
        &address.to_string(),
        &[address],
        "/face/export/snapshot",
        started,
        started + FETCH_BUDGET_MS,
    );
    assert!(
        result.is_err(),
        "late 200 response must not restart the fetch budget"
    );
    server.join().unwrap();
}
