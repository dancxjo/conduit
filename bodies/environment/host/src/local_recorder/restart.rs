// CPU fixture: no real membership, hardware reboot or peer recording is claimed.
#[test]
fn private_joined_binding_recovers_old_and_new_producer_boot_after_outage() {
    use ed25519_dalek::Signer;
    use sha2::{Digest, Sha256};
    use std::io::{Read, Write};
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
    // Reuse the actual SDK-independent producer wire, without widening the
    // production history module's private API just for this sibling test.
    #[allow(dead_code, clippy::duplicate_mod)]
    #[path = "../../../../../targets/esp32/proof/tm1637/src/native_observed_wire.rs"]
    mod history_wire;
    let (key, receipt) = receipt();
    let directory =
        std::env::temp_dir().join(format!("conduit-joined-reboot-{}", std::process::id()));
    if directory.exists() {
        std::fs::remove_dir_all(&directory).unwrap();
    }
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&directory)
        .unwrap();
    let binding = serde_json::to_vec(&serde_json::json!({
        "schema":"conduit.environment/joined-recorder@1",
        "invitation":{"invitation_id":receipt.invitation_id,"body_id":receipt.credential.body_id,
            "nonce":vec![3;32],"expires_at_millis":999},
        "root_key":key.verifying_key().to_bytes(),"local_host":receipt.credential.host_id,
        "local_boot":receipt.credential.boot_id,"receipt":receipt,
    }))
    .unwrap();
    let path = directory.join("binding.json");
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .unwrap()
        .write_all(&binding)
        .unwrap();
    let collector = admitted_collector(&path).unwrap();
    let mut old = [0; 96];
    old[..32].copy_from_slice(&Sha256::digest(
        receipt.credential.body_id.as_str().as_bytes(),
    ));
    old[32..64].copy_from_slice(&Sha256::digest(receipt.producer_boot.as_str().as_bytes()));
    old[64..].fill(9);
    let mut new = old;
    new[32..64].copy_from_slice(&Sha256::digest(b"fresh-producer-Boot"));
    new[64..].fill(10);
    let contract =
        conduit_environment_runtime::observation::ObservationContract::prepare().unwrap();
    let forms: [u8; 32] = Sha256::digest(
        conduit_environment_runtime::observed_record::descriptor(&contract).unwrap(),
    )
    .into();
    let hex = |bytes: &[u8]| bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
    let mut raw = [0; history_wire::DIRECTORY_BYTES];
    raw[..32].copy_from_slice(&old[..32]);
    raw[32] = 2;
    for (entry, (slot, scope)) in raw[33..299]
        .as_chunks_mut::<133>()
        .0
        .iter_mut()
        .zip([(0, new), (3, old)])
    {
        entry[0] = slot;
        entry[1..97].copy_from_slice(&scope);
        entry[97..129].copy_from_slice(&forms);
        entry[129..].copy_from_slice(&1u32.to_le_bytes());
    }
    let listing = |signer: &ed25519_dalek::SigningKey,
                   bytes: &[u8; history_wire::DIRECTORY_BYTES]| {
        serde_json::to_vec(&serde_json::json!({
            "profile":"observed-epochs@1","directory":hex(bytes),
            "signature":hex(&signer.sign(&history_wire::directory_transcript(bytes)).to_bytes())
        }))
        .unwrap()
    };
    let batch = |scope: [u8; 96], after, clock| {
        let record = conduit_environment_runtime::observed_record::ObservedRecord {
            sequence: 1,
            observed_at_boot_ms: clock,
            disposition:
                conduit_environment_runtime::recording::RecordingDisposition::ProviderFailed,
            quantities: None,
        }
        .encode()
        .unwrap();
        let records = if after == 0 { record.as_slice() } else { &[] };
        serde_json::to_vec(&serde_json::json!({"profile":"observed-history@1","scope":hex(&scope),"forms":hex(&forms),
            "after":after,"committed":1,"records":hex(records),
            "signature":hex(&key.sign(&history_wire::transcript(&scope,&forms,after,1,records)).to_bytes())})).unwrap()
    };
    let mut foreign = raw;
    foreign[0] ^= 1;
    let responses = vec![
        None, // A real TCP interruption must leave binding and custody unchanged.
        Some(listing(
            &ed25519_dalek::SigningKey::from_bytes(&[99; 32]),
            &raw,
        )),
        Some(listing(&key, &foreign)),
        Some(listing(&key, &raw)),
        Some(batch(new, 0, 500)),
        Some(batch(new, 1, 500)),
        Some(batch(old, 0, 90_000)),
        Some(batch(old, 1, 90_000)),
        Some(listing(&key, &raw)),
        Some(batch(new, 1, 500)),
        Some(batch(old, 1, 90_000)),
    ];
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let worker = std::thread::spawn(move || {
        for response in responses {
            let (mut socket, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                let mut b = [0];
                socket.read_exact(&mut b).unwrap();
                request.push(b[0]);
            }
            if let Some(body) = response {
                socket
                    .write_all(b"HTTP/1.0 200 OK\r\nConnection: close\r\n\r\n")
                    .unwrap();
                socket.write_all(&body).unwrap();
            }
        }
    });
    let custody = directory.join("history");
    for _ in 0..3 {
        assert!(collector.collect_epochs(address, &custody).is_err());
        assert!(!custody.exists());
    }
    assert_eq!(collector.collect_epochs(address, &custody).unwrap(), 2);
    // Restart the collector from the SAME private admission, without relabeling it.
    assert_eq!(
        admitted_collector(&path)
            .unwrap()
            .collect_epochs(address, &custody)
            .unwrap(),
        2
    );
    worker.join().unwrap();
    assert_eq!(std::fs::read(&path).unwrap(), binding);
    for scope in [old, new] {
        let epoch = custody.join(format!("epoch-{}", hex(&scope)));
        let mut recovered =
            Collector::from_authenticated_scope(scope, &key.verifying_key().to_bytes()).unwrap();
        recovered.restore(&epoch).unwrap();
        assert_eq!(recovered.after, 1);
        assert_eq!(std::fs::read_dir(epoch).unwrap().count(), 1);
    }
    std::fs::remove_dir_all(directory).unwrap();
}
