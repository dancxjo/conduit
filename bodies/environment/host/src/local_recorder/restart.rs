// CPU canonical Body admission and host custody; no physical peer/reboot claim.
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
    let directory =
        std::env::temp_dir().join(format!("conduit-joined-reboot-{}", std::process::id()));
    if directory.exists() {
        std::fs::remove_dir_all(&directory).unwrap();
    }
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&directory)
        .unwrap();
    let (key, receipt, claim) = canonical_admission(&directory);
    let binding = serde_json::to_vec(&serde_json::json!({
        "schema":"conduit.environment/joined-recorder@1",
        "invitation":claim,
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
    // All explicit routes fail authentication/transport; none may create a
    // durable batch, nor may the collection-first caller enter custody work.
    for _ in 0..3 {
        assert!(collect_routes(&collector, &[&address.to_string()], 0, &custody).is_err());
        assert!(!custody.exists());
    }
    // Run the actual joined recorder entry in two separate OS processes. Both
    // reload the SAME private admission and derive cursors from fsynced files.
    for _ in 0..2 {
        assert!(std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "local_recorder::tests::joined_recorder_process_helper"
            ])
            .env("CONDUIT_TEST_RECORDER_BINDING", &path)
            .env("CONDUIT_TEST_RECORDER_HISTORY", &custody)
            .env("CONDUIT_TEST_RECORDER_PEER", address.to_string())
            .status()
            .unwrap()
            .success());
    }
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

#[test]
fn joined_recorder_process_helper() {
    let Some(binding) = std::env::var_os("CONDUIT_TEST_RECORDER_BINDING") else {
        return;
    };
    let history = std::env::var_os("CONDUIT_TEST_RECORDER_HISTORY").unwrap();
    let peer = std::env::var("CONDUIT_TEST_RECORDER_PEER").unwrap();
    // Reserve then close a real socket to provide a refused primary route.
    let refused = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let refused_peer = refused.local_addr().unwrap().to_string();
    drop(refused);
    use std::io::{Read, Write};
    use ed25519_dalek::Signer;
    use sha2::{Digest, Sha256};
    #[allow(dead_code, clippy::duplicate_mod)]
    #[path = "../../../../../targets/esp32/proof/tm1637/src/native_observed_wire.rs"]
    mod route_wire;
    let foreign = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let foreign_peer = foreign.local_addr().unwrap().to_string();
    let binding_scope = validated_binding(Path::new(&binding)).unwrap();
    let mut raw = [0; route_wire::DIRECTORY_BYTES];
    raw[..32].copy_from_slice(&Sha256::digest(binding_scope.credential().body_id.as_str().as_bytes()));
    let wrong_root = ed25519_dalek::SigningKey::from_bytes(&[99; 32]);
    let hex = |bytes: &[u8]| bytes.iter().map(|b| format!("{b:02x}")).collect::<String>();
    let response = serde_json::to_vec(&serde_json::json!({
        "profile":"observed-epochs@1", "directory":hex(&raw),
        "signature":hex(&wrong_root.sign(&route_wire::directory_transcript(&raw)).to_bytes())
    })).unwrap();
    let foreign_server = std::thread::spawn(move || {
        let (mut socket, _) = foreign.accept().unwrap();
        let mut request = Vec::new();
        while !request.ends_with(b"\r\n\r\n") {
            let mut byte = [0];
            socket.read_exact(&mut byte).unwrap();
            request.push(byte[0]);
        }
        assert!(std::str::from_utf8(&request).unwrap().starts_with("GET /history?after=4294967295 "));
        socket.write_all(b"HTTP/1.0 200 OK\r\nConnection: close\r\n\r\n").unwrap();
        socket.write_all(&response).unwrap();
    });
    let list = format!("{refused_peer},{foreign_peer},{peer}");
    let peers = continuing_peers(&list).unwrap();
    let collector = admitted_collector(Path::new(&binding)).unwrap();
    run_loop(collector, peers, Path::new(&history), true, None).unwrap();
    foreign_server.join().unwrap();
}

pub(super) fn canonical_admission(
    directory: &Path,
) -> (
    ed25519_dalek::SigningKey,
    joined::JoinedHostReceipt,
    conduit_body::SpawnInvitationClaim,
) {
    use conduit_body::{AdmissionManager, SpawnAdmissionProof, SpawnInvitationSecret};
    use ed25519_dalek::Signer;
    use std::{io::Write, os::unix::fs::OpenOptionsExt};
    let key = ed25519_dalek::SigningKey::from_bytes(&[41; 32]);
    let root = conduit_core::HostAdvertisement {
        protocol_version: conduit_core::PROTOCOL_VERSION,
        host_id: "producer/root".into(),
        boot_id: "producer/boot-one".into(),
        offer_generation: conduit_core::OfferGeneration(1),
        profile: "fixture/root".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    let sign = conduit_core::bind_sign(&root.host_id, &root.boot_id, None, 0);
    let image =
        conduit_environment_make::EnvironmentBodyImage::prepare_mask_from_sign(&sign).unwrap();
    let mut owner = conduit_environment_runtime::body_image::open_installed_image_sealed(
        image.body.evidence().clone(),
        &root,
    )
    .unwrap();
    let mut manager = AdmissionManager::new(owner.evidence().body_id.clone()).unwrap();
    let secret = SpawnInvitationSecret::from_csprng_bytes([22; 32]).unwrap();
    let claim = owner
        .issue_invitation(
            &mut manager,
            secret.clone(),
            [3; 32],
            10,
            999,
            &root.host_id,
            &root.boot_id,
        )
        .unwrap();
    let observer = conduit_core::HostAdvertisement {
        host_id: "observer/host".into(),
        boot_id: "observer/boot".into(),
        ..root.clone()
    };
    let proof = SpawnAdmissionProof {
        invitation_id: claim.invitation_id.clone(),
        body_id: claim.body_id.clone(),
        host_id: observer.host_id.clone(),
        boot_id: observer.boot_id.clone(),
        nonce: claim.nonce,
        signature: secret.sign(&claim.signing_transcript(
            &observer.host_id,
            &observer.boot_id,
            observer.offer_generation,
        )),
    };
    let credential = owner
        .admit_invited_host(
            &mut manager,
            &observer,
            &proof,
            11,
            &root.host_id,
            &root.boot_id,
        )
        .unwrap();
    assert!(owner
        .admit_invited_host(
            &mut manager,
            &observer,
            &proof,
            12,
            &root.host_id,
            &root.boot_id
        )
        .is_err());
    // This CPU owner writes its complete actual membership+replay manager before
    // signing admission. It is not the native flash checkpoint implementation.
    let checkpoint = directory.join("canonical-body-checkpoint.json");
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(&checkpoint)
        .unwrap();
    serde_json::to_writer(&mut file, &(owner.evidence(), &manager)).unwrap();
    file.flush().unwrap();
    file.sync_all().unwrap();
    std::fs::File::open(directory).unwrap().sync_all().unwrap();
    let (evidence, recovered): (conduit_body::BodyBiographyEvidence, AdmissionManager) =
        serde_json::from_slice(&std::fs::read(checkpoint).unwrap()).unwrap();
    evidence.validate().unwrap();
    assert_eq!(evidence, *owner.evidence());
    assert_eq!(recovered, manager);
    recovered
        .verify_admitted_message(
            &evidence.membership,
            &credential.part_id,
            &observer.host_id,
            &observer.boot_id,
            &[42; 32],
            &secret.sign(&[42; 32]),
        )
        .unwrap();
    let mut receipt = joined::JoinedHostReceipt {
        schema: joined::PROFILE.into(),
        credential,
        membership_revision: evidence.membership.revision.0,
        producer_host: root.host_id,
        producer_boot: root.boot_id,
        producer_key: key.verifying_key().to_bytes(),
        invitation_id: claim.invitation_id.clone(),
        offer_generation: observer.offer_generation,
        signature: vec![],
    };
    receipt.signature = key
        .sign(&joined::transcript(&receipt.scope()))
        .to_bytes()
        .to_vec();
    (key, receipt, claim)
}
