use super::*;
use conduit_body as b;
use conduit_core as c;
use ed25519_dalek::Signer;
fn archives() -> Vec<BodyBiographyArchiveSegment> {
    let workset = b::BodyWorkset::from_plots([b::ResidentPlot::new(
        "source/test".into(),
        "checked/test".into(),
    )])
    .unwrap();
    let body = b::Body::born_with_plots(workset, 1, "sign/birth".into()).unwrap();
    let membership = b::BodyMembership::new(body.body_id.clone()).unwrap();
    let evidence =
        b::BodyBiographyEvidence::born(body, membership, "Custody Fixture".into()).unwrap();
    let host = c::HostAdvertisement {
        protocol_version: c::PROTOCOL_VERSION,
        host_id: "host/custody".into(),
        boot_id: "boot/0".into(),
        offer_generation: c::OfferGeneration(1),
        profile: "fixture/custody".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    let mut body =
        conduit_environment_runtime::body_image::open_installed_image(evidence, &host).unwrap();
    let mut archives = Vec::new();
    for i in 1..=120 {
        body = b::BodyLifecycleSession::resume_here(
            body.evidence().clone(),
            &host.host_id,
            &format!("boot/{i}").into(),
        )
        .unwrap();
        if !body.pending_archives().is_empty() {
            archives.extend_from_slice(body.pending_archives());
            let head = body.pending_archives().last().unwrap().digest;
            body.acknowledge_archives(head).unwrap();
            if archives.len() >= 3 {
                return archives;
            }
        }
    }
    panic!("fixture failed to cross the actual membership compaction boundary")
}

fn directory() -> std::path::PathBuf {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    std::env::temp_dir().join(format!(
        "conduit-remote-biography-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    ))
}
fn offer(
    segments: &[BodyBiographyArchiveSegment],
    base: wire::Summary,
) -> (wire::Offer, Vec<u8>, ed25519_dalek::SigningKey) {
    let key = ed25519_dalek::SigningKey::from_bytes(&[17; 32]);
    let body = segments[0].body_id.clone();
    let mut end = base.clone();
    for segment in segments {
        end.append(&body, segment).unwrap();
    }
    let stream = serde_json::to_vec(segments).unwrap();
    let mut offer = wire::Offer {
        schema: wire::OFFER.into(),
        body,
        generation: 1,
        source: wire::Hex([19; 32]),
        base,
        end,
        stream: wire::Hex(Sha256::digest(&stream).into()),
        signature: wire::Hex([0; 64]),
    };
    offer.signature = wire::Hex(
        key.sign(&offer.transcript(&key.verifying_key().to_bytes()))
            .to_bytes(),
    );
    (offer, stream, key)
}
#[test]
fn actual_compaction_chain_survives_two_exports_and_reopen() {
    let segments = archives();
    let dir = directory();
    let (first, stream, key) = offer(&segments[..1], wire::Summary::default());
    let root = key.verifying_key().to_bytes();
    let token =
        VerifiedDurableBiography::retain(&first, &first.body, &root, &stream, &dir).unwrap();
    assert_eq!(token.root(), &root);
    assert_eq!(token.body(), &first.body);
    let credential = MembershipCredential {
        credential_id: serde_json::from_value(serde_json::json!("credential/test")).unwrap(),
        body_id: first.body.clone(),
        part_id: serde_json::from_value(serde_json::json!("part/test")).unwrap(),
        host_id: "host/test".into(),
        boot_id: "boot/test".into(),
        issued_at_millis: 0,
    };
    let signature = key
        .sign(&token.signing_transcript(&credential).unwrap())
        .to_bytes();
    let request: wire::Request =
        serde_json::from_slice(&token.encode_ack(&credential, signature).unwrap()).unwrap();
    assert_eq!(request.offer.0, first.transcript(&root));
    let (second, stream, _) = offer(&segments[1..], first.end.clone());
    VerifiedDurableBiography::retain(&second, &second.body, &root, &stream, &dir).unwrap();
    verify_files(&dir, &second).unwrap();
    // A different admitted-history directory cannot claim an already offloaded base.
    let absent = directory();
    assert!(
        VerifiedDurableBiography::retain(&second, &second.body, &root, &stream, &absent).is_err()
    );
    // Reopening catches removed predecessors even if the final immutable manifest exists.
    std::fs::remove_file(dir.join(format!("{}.json", name(&segments[0].digest)))).unwrap();
    assert!(verify_files(&dir, &second).is_err());
    std::fs::remove_dir_all(dir).unwrap();
    std::fs::remove_dir_all(absent).unwrap();
}
#[test]
fn forged_offer_corrupt_stream_and_conflicting_file_never_mint_token() {
    let segments = archives();
    let (dir, other) = (directory(), directory());
    let (mut offered, stream, key) = offer(&segments[..1], wire::Summary::default());
    let root = key.verifying_key().to_bytes();
    offered.signature.0[0] ^= 1;
    assert!(
        VerifiedDurableBiography::retain(&offered, &offered.body, &root, &stream, &dir).is_err()
    );
    assert!(!dir.exists());
    offered.signature.0[0] ^= 1;
    let mut corrupt = stream.clone();
    corrupt[0] = b' ';
    assert!(
        VerifiedDurableBiography::retain(&offered, &offered.body, &root, &corrupt, &dir).is_err()
    );
    assert!(!dir.exists());
    create_durable(&other).unwrap();
    let file = other.join(format!("{}.json", name(&segments[0].digest)));
    Directory::open(&other)
        .unwrap()
        .immutable(file.file_name().unwrap().to_str().unwrap(), b"conflict")
        .unwrap();
    assert!(
        VerifiedDurableBiography::retain(&offered, &offered.body, &root, &stream, &other).is_err()
    );
    assert_eq!(std::fs::read(file).unwrap(), b"conflict");
    std::fs::remove_dir_all(other).unwrap();
}

#[test]
fn canonical_admitted_ack_and_independently_pinned_receipt() {
    let workset = b::BodyWorkset::from_plots([b::ResidentPlot::new(
        "source/test".into(),
        "checked/test".into(),
    )])
    .unwrap();
    let body = b::Body::born_with_plots(workset, 1, "sign/birth".into()).unwrap();
    let membership = b::BodyMembership::new(body.body_id.clone()).unwrap();
    let evidence =
        b::BodyBiographyEvidence::born(body, membership, "Custody Fixture".into()).unwrap();
    let host = c::HostAdvertisement {
        protocol_version: c::PROTOCOL_VERSION,
        host_id: "host/custody".into(),
        boot_id: "boot/0".into(),
        offer_generation: c::OfferGeneration(1),
        profile: "fixture/custody".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };

    let mut owner =
        conduit_environment_runtime::body_image::open_installed_image_sealed(evidence, &host)
            .unwrap();
    let mut manager = b::AdmissionManager::new(owner.evidence().body_id.clone()).unwrap();
    let secret = b::SpawnInvitationSecret::from_csprng_bytes([22; 32]).unwrap();
    let claim = owner
        .issue_invitation(
            &mut manager,
            secret.clone(),
            [3; 32],
            10,
            999,
            &host.host_id,
            &host.boot_id,
        )
        .unwrap();
    let observer = c::HostAdvertisement {
        host_id: "observer/host".into(),
        boot_id: "observer/boot".into(),
        ..host.clone()
    };
    let proof = b::SpawnAdmissionProof {
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
            &host.host_id,
            &host.boot_id,
        )
        .unwrap();
    let segments = archives();
    let (offer, stream, key) = offer(&segments[..1], wire::Summary::default());
    assert_eq!(offer.body, owner.evidence().body_id);
    let dir = directory();
    let root = key.verifying_key();
    let token =
        VerifiedDurableBiography::retain(&offer, &offer.body, &root.to_bytes(), &stream, &dir)
            .unwrap();
    let signature = secret.sign(&token.signing_transcript(&credential).unwrap());
    let bytes = token.encode_ack(&credential, signature).unwrap();
    let mut request: wire::Request = serde_json::from_slice(&bytes).unwrap();
    let draft = acceptance::verify_ack(
        &offer,
        &root,
        &manager,
        &owner.evidence().membership,
        &request,
    )
    .unwrap();
    assert_eq!(draft.root(), &root.to_bytes());
    let signed = key.sign(&draft.transcript()).to_bytes();
    let receipt = draft.signed(signed);
    receipt.verify(&offer.body, &root).unwrap();
    let foreign = ed25519_dalek::SigningKey::from_bytes(&[99; 32]).verifying_key();
    assert!(receipt.verify(&offer.body, &foreign).is_err());
    let mut tampered = receipt.clone();
    tampered.summary.count += 1;
    assert!(tampered.verify(&offer.body, &root).is_err());
    request.signature.0[0] ^= 1;
    assert!(acceptance::verify_ack(
        &offer,
        &root,
        &manager,
        &owner.evidence().membership,
        &request
    )
    .is_err());
    request.signature.0[0] ^= 1;
    request.boot = "stale/boot".into();
    assert!(acceptance::verify_ack(
        &offer,
        &root,
        &manager,
        &owner.evidence().membership,
        &request
    )
    .is_err());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn owner_bootstrap_is_typed_scope_bound_and_cold_receipt_pins_root() {
    use crate::native_body_join::owner;
    let segments = archives();
    let (offered, stream, rootkey) = offer(&segments[..1], wire::Summary::default());
    let root = rootkey.verifying_key();
    let id = offered.transcript(&root.to_bytes());
    let dir = directory();
    let token =
        VerifiedDurableBiography::retain(&offered, &offered.body, &root.to_bytes(), &stream, &dir)
            .unwrap();
    let ownerkey = ed25519_dalek::SigningKey::from_bytes(&[42; 32]);
    let mut gate = owner::Gate::new(Some(ownerkey.verifying_key().to_bytes())).unwrap();
    gate.challenge([31; 32], 100).unwrap();
    let challenge = owner::BiographyChallenge {
        host: "root/host".into(),
        boot: "root/boot".into(),
        body: offered.body.as_str().into(),
        root_key: root.to_bytes(),
        nonce: [31; 32],
        sequence: 1,
        offer: id,
    };
    for changed in 0..3 {
        let mut c = challenge.clone();
        match changed {
            0 => c.body = "foreign/body".into(),
            1 => c.root_key = [0; 32],
            _ => c.offer = [1; 32],
        };
        assert!(token.owner_draft(c).is_err());
    }
    let draft = token.owner_draft(challenge.clone()).unwrap();
    let signature = ownerkey
        .sign(&draft.signing_transcript().unwrap())
        .to_bytes();
    let bytes = draft.encode(signature).unwrap();
    let decode = || serde_json::from_slice::<owner::BiographyRequest>(&bytes).unwrap();
    for changed in 0..8 {
        let mut req = decode();
        match changed {
            0 => req.challenge.boot = "stale/boot".into(),
            1 => req.challenge.body = "foreign/body".into(),
            2 => req.challenge.root_key = [0; 32],
            3 => req.challenge.offer = [2; 32],
            4 => req.challenge.nonce = [2; 32],
            5 => req.schema = "conduit.environment/private-invitation-owner@1".into(),
            6 => req.signature[0] ^= 1,
            _ => req.challenge.host = "foreign/host".into(),
        };
        assert!(gate
            .authorize_biography(
                &req,
                "root/host",
                "root/boot",
                offered.body.as_str(),
                &root.to_bytes(),
                &id,
                101
            )
            .is_err());
    }
    assert!(gate
        .authorize_biography(
            &decode(),
            "root/host",
            "root/boot",
            offered.body.as_str(),
            &root.to_bytes(),
            &id,
            30101
        )
        .is_err());
    let proof = gate
        .authorize_biography(
            &decode(),
            "root/host",
            "root/boot",
            offered.body.as_str(),
            &root.to_bytes(),
            &id,
            101,
        )
        .unwrap();
    assert_eq!(gate.next_sequence(), Some(2));
    assert!(gate
        .authorize_biography(
            &decode(),
            "root/host",
            "root/boot",
            offered.body.as_str(),
            &root.to_bytes(),
            &id,
            101
        )
        .is_err());
    let accepted = acceptance::accept_owner(&offered, &root, proof).unwrap();
    let signature = rootkey.sign(&accepted.transcript()).to_bytes();
    let receipt = accepted.signed(signature);
    receipt.verify(&offered.body, &root).unwrap();
    assert!(matches!(receipt.consumer, wire::Consumer::Owner { .. }));
    let encoded = serde_json::to_vec(&receipt).unwrap();
    let cold: wire::Receipt = serde_json::from_slice(&encoded).unwrap();
    cold.verify(&offered.body, &root).unwrap();
    let mut corrupt = cold.clone();
    corrupt.consumer = wire::Consumer::Admitted {
        identity: wire::Hex([0; 32]),
    };
    assert!(corrupt.verify(&offered.body, &root).is_err());
    assert!(cold
        .verify(&offered.body, &ownerkey.verifying_key())
        .is_err());
    // Fresh nonce does not allow a previously accepted sequence, even signed.
    gate.challenge([32; 32], 200).unwrap();
    let mut req = decode();
    req.challenge.nonce = [32; 32];
    req.signature = ownerkey
        .sign(&req.transcript().unwrap())
        .to_bytes()
        .to_vec();
    assert!(gate
        .authorize_biography(
            &req,
            "root/host",
            "root/boot",
            offered.body.as_str(),
            &root.to_bytes(),
            &id,
            201
        )
        .is_err());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn biography_extension_preserves_invitation_gate_domain_and_replay() {
    use crate::native_body_join::owner;
    let key = ed25519_dalek::SigningKey::from_bytes(&[43; 32]);
    let mut gate = owner::Gate::new(Some(key.verifying_key().to_bytes())).unwrap();
    gate.challenge([8; 32], 100).unwrap();
    let mut req = owner::Request {
        schema: owner::PROFILE.into(),
        host: "host/a".into(),
        boot: "boot/a".into(),
        body: "body/a".into(),
        root_key: [7; 32],
        nonce: [8; 32],
        sequence: 1,
        ttl_ms: 1000,
        signature: Vec::new(),
    };
    req.signature = key.sign(&req.transcript()).to_bytes().to_vec();
    let authorized = gate
        .authorize(&req, "host/a", "boot/a", "body/a", &[7; 32], 101)
        .unwrap();
    assert_eq!(authorized.ttl_ms(), 1000);
    assert!(gate
        .authorize(&req, "host/a", "boot/a", "body/a", &[7; 32], 101)
        .is_err());
    assert_eq!(gate.next_sequence(), Some(2));
}

#[test]
fn custody_refuses_public_directories_files_and_symlinks_and_reuses_private_files() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let segments = archives();
    let (offered, stream, key) = offer(&segments[..1], wire::Summary::default());
    let root = key.verifying_key().to_bytes();
    let dir = directory();
    std::fs::create_dir(&dir).unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert!(
        VerifiedDurableBiography::retain(&offered, &offered.body, &root, &stream, &dir).is_err()
    );
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 0);
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    let alias = directory();
    symlink(&dir, &alias).unwrap();
    assert!(
        VerifiedDurableBiography::retain(&offered, &offered.body, &root, &stream, &alias).is_err()
    );
    std::fs::remove_file(alias).unwrap();
    let file = dir.join(format!("{}.json", name(&segments[0].digest)));
    let bytes = serde_json::to_vec(&segments[0]).unwrap();
    std::fs::write(&file, &bytes).unwrap();
    std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert!(
        VerifiedDurableBiography::retain(&offered, &offered.body, &root, &stream, &dir).is_err()
    );
    std::fs::remove_file(&file).unwrap();
    let target = directory();
    std::fs::write(&target, &bytes).unwrap();
    symlink(&target, &file).unwrap();
    assert!(
        VerifiedDurableBiography::retain(&offered, &offered.body, &root, &stream, &dir).is_err()
    );
    std::fs::remove_file(file).unwrap();
    std::fs::remove_file(target).unwrap();
    VerifiedDurableBiography::retain(&offered, &offered.body, &root, &stream, &dir).unwrap();
    VerifiedDurableBiography::retain(&offered, &offered.body, &root, &stream, &dir).unwrap();
    verify_files(&dir, &offered).unwrap();
    for entry in std::fs::read_dir(&dir).unwrap() {
        assert_eq!(
            entry.unwrap().metadata().unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    std::fs::remove_dir_all(dir).unwrap();
    let nested = directory();
    let leaf = nested.join("a/b");
    VerifiedDurableBiography::retain(&offered, &offered.body, &root, &stream, &leaf).unwrap();
    for path in [&nested, &nested.join("a"), &leaf] {
        assert_eq!(
            std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }
    std::fs::remove_dir_all(nested).unwrap();
}
