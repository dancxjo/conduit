use super::*;

fn fixture() -> (MemberContact, MembershipCredential) {
    let credential = MembershipCredential {
        credential_id: "credential/member".into(),
        body_id: "body/contact".into(),
        part_id: "part/member".into(),
        host_id: "host/member".into(),
        boot_id: "boot/member/1".into(),
        issued_at_millis: 1,
    };
    let contact = MemberContact {
        schema: SCHEMA.into(),
        body_id: credential.body_id.clone(),
        part_id: credential.part_id.clone(),
        host_id: credential.host_id.clone(),
        owner: FederationPeer {
            host_id: "host/owner".into(),
            boot_id: "boot/owner/1".into(),
            offer_generation: conduit_core::OfferGeneration(1),
            verifying_key: [7; 32],
        },
        reachability: "wss://192.0.2.1:4443".into(),
        authentication: RendezvousAuthentication {
            server_identity: "192.0.2.1".into(),
            transport_binding_sha256: [3; 32],
        },
        reconnect_after_millis: 1_000,
        attempt_timeout_millis: 3_000,
        maximum_attempts: 2,
    };
    (contact, credential)
}

#[test]
fn standing_contact_has_no_execution_grant_and_requires_exact_scope_and_bounds() {
    let (contact, credential) = fixture();
    contact.validate(&credential).unwrap();
    let encoded = serde_json::to_value(&contact).unwrap();
    assert!(encoded.get("expires_at_millis").is_none());
    assert!(encoded.get("invitation").is_none());
    let mut wrong = credential.clone();
    wrong.part_id = "part/other".into();
    assert!(contact.validate(&wrong).is_err());
    for (attempts, timeout, delay) in [
        (0, 3_000, 1_000),
        (3, 3_000, 1_000),
        (1, 0, 1_000),
        (1, 3_001, 1_000),
        (1, 3_000, 0),
    ] {
        let mut bad = contact.clone();
        bad.maximum_attempts = attempts;
        bad.attempt_timeout_millis = timeout;
        bad.reconnect_after_millis = delay;
        assert!(bad.validate(&credential).is_err());
    }
}

#[test]
fn contact_cannot_refresh_changed_owner_or_expired_evidence() {
    let (contact, _) = fixture();
    let now = current_time_millis().unwrap();
    let evidence = contact.encounter(30).unwrap();
    contact.validate_evidence(&evidence, now).unwrap();
    let mut bad = evidence.clone();
    bad.owner.boot_id = "boot/owner/replaced".into();
    assert!(contact.validate_evidence(&bad, now).is_err());
    let mut bad = evidence.clone();
    bad.candidate.expires_at_millis = now;
    assert!(contact.validate_evidence(&bad, now).is_err());
    let mut bad = evidence.clone();
    bad.candidate.expires_at_millis = now + 600_001;
    assert!(contact.validate_evidence(&bad, now).is_err());
    let mut bad = evidence;
    bad.candidate.authentication.transport_binding_sha256[0] ^= 1;
    assert!(contact.validate_evidence(&bad, now).is_err());
}

fn tls_discovery_case(wrong_nonce: bool) {
    let (mut contact, _) = fixture();
    let root = std::env::temp_dir().join(crate::durable_host::fresh_identity(
        "member-contact-test",
        "tls",
    ));
    std::fs::create_dir_all(&root).unwrap();
    let rcgen::CertifiedKey { cert, signing_key } =
        rcgen::generate_simple_self_signed(vec!["127.0.0.1".into()]).unwrap();
    let certificate = root.join("cert.pem");
    let private = root.join("key.pem");
    std::fs::write(&certificate, cert.pem()).unwrap();
    std::fs::write(&private, signing_key.serialize_pem()).unwrap();
    let listener = SecureWebSocketListener::bind(
        "127.0.0.1:0".parse().unwrap(),
        &certificate,
        &private,
        MAXIMUM_ROUTE_FRAME_BYTES as u32,
        true,
    )
    .unwrap();
    contact.reachability = format!("wss://{}", listener.local_addr().unwrap());
    contact.authentication.transport_binding_sha256 = listener.certificate_binding_sha256();
    contact.authentication.server_identity = "127.0.0.1".into();
    let address = listener.local_addr().unwrap();
    let expected = contact.clone();
    let worker = std::thread::spawn(move || {
        let mut line = listener
            .accept_with_timeout(Duration::from_secs(5))
            .unwrap();
        let MemberReturnFrame::DescribeContact { schema, mut nonce } = receive(&mut line).unwrap()
        else {
            panic!("expected discovery, not admission")
        };
        assert_eq!(schema, SCHEMA);
        if wrong_nonce {
            nonce[0] ^= 1;
        }
        send(
            &mut line,
            &MemberReturnFrame::ContactDescribed {
                schema: SCHEMA.into(),
                nonce,
                encounter: Box::new(expected.encounter(30).unwrap()),
            },
        )
        .unwrap();
        let _ = line.close();
    });
    // Loopback exercises the private pinned transport seam, not eligibility for
    // an installed standing contact. Production resolve rejects loopback first.
    let result = resolve_at(&contact, address);
    if wrong_nonce {
        assert!(result.err().unwrap().contains("nonce"));
    } else {
        contact
            .validate_evidence(&result.unwrap(), current_time_millis().unwrap())
            .unwrap();
    }
    worker.join().unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn pinned_tls_discovery_returns_new_finite_evidence_without_membership() {
    tls_discovery_case(false);
}
#[test]
fn pinned_tls_discovery_refuses_a_different_nonce() {
    tls_discovery_case(true);
}

#[test]
fn standing_contact_rejects_unbounded_dns_and_nonremote_locators() {
    let (contact, credential) = fixture();
    for endpoint in [
        "wss://owner.local:4443",
        "wss://127.0.0.1:4443",
        "wss://0.0.0.0:4443",
        "wss://192.0.2.1:0",
    ] {
        let mut bad = contact.clone();
        bad.reachability = endpoint.into();
        assert!(bad.validate(&credential).is_err());
    }
}
