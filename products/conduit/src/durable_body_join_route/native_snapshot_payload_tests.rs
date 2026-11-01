//! Authenticated transport does not excuse payloads contrary to the selected Body.
use super::*;
#[test]
fn real_protected_peer_wrong_schema_or_body_is_refused() {
    for wrong_schema in [false, true] {
        let mut f = fixture();
        let previous = f.client.as_ref().unwrap().advertisement().clone();
        let private_before =
            fs::read(f.client_control.root.join("body/native-part-private.json")).unwrap();
        let verified = control::native_snapshot::start(
            &f.owner.root,
            "host/snapshot-client",
            10_000,
            &f.selected,
            &f.route,
        )
        .unwrap();
        let (client, mut server) = native_snapshot_lines(f.selected.clone(), 2 * 1024 * 1024);
        std::thread::scope(|scope| {
            let worker = scope.spawn(|| {
                let binding = LinkBindingId::from(format!(
                    "binding/native-protected-snapshot/{}",
                    f.selected.line_session_id
                ));
                let first = server.receive().unwrap();
                let challenge = browser::begin(
                    &f.owner.root,
                    &verified.authorization().window_id,
                    binding.clone(),
                    serde_json::from_slice(&first).unwrap(),
                    first.len() as u32,
                )
                .unwrap();
                server
                    .send(&serde_json::to_vec(&challenge).unwrap())
                    .unwrap();
                let proof = server.receive().unwrap();
                let BrowserAdmissionCompletion::Active { snapshot } = browser::complete(
                    &f.owner.root,
                    &verified.authorization().window_id,
                    serde_json::from_slice(&proof).unwrap(),
                )
                .unwrap() else {
                    panic!("expected actual owner completion")
                };
                server
                    .send(
                        &serde_json::to_vec(&Out::Admitted {
                            protocol: PROTOCOL,
                            credential: snapshot.credential.clone(),
                            signed_membership_receipt: None,
                        })
                        .unwrap(),
                    )
                    .unwrap();
                let In::FaceSnapshotRequest {
                    protocol: PROTOCOL,
                    request,
                } = serde_json::from_slice(&server.receive().unwrap()).unwrap()
                else {
                    panic!("expected canonical Face request")
                };
                let mut response = control::native_snapshot::face(
                    &f.owner.root,
                    &verified.authorization().window_id,
                    binding,
                    request,
                )
                .unwrap();
                let OwnerFaceSnapshotResponse::Snapshot { schema, face, .. } = &mut response else {
                    panic!("expected actual owner Face")
                };
                if wrong_schema {
                    *schema = "foreign/snapshot-schema".into();
                } else {
                    face.basis.body_id = Some(
                        Body::born(
                            "source/other-body".into(),
                            "checked/other-body".into(),
                            1,
                            "sign/other-body".into(),
                        )
                        .unwrap()
                        .body_id,
                    );
                }
                server
                    .send(
                        &serde_json::to_vec(&Out::FaceSnapshotResponse {
                            protocol: PROTOCOL,
                            response,
                        })
                        .unwrap(),
                    )
                    .unwrap();
                browser::leave(
                    &f.owner.root,
                    &verified.authorization().window_id,
                    snapshot.credential,
                )
                .unwrap();
                browser::cancel(&f.owner.root, &verified.authorization().window_id).unwrap();
                server.close().unwrap();
            });
            assert!(native_snapshot(f.client.take().unwrap(), &verified, client).is_err());
            worker.join().unwrap();
        });
        let reopened = RetainedPart::open(&f.client_control.root, previous).unwrap();
        let original: serde_json::Value = serde_json::from_slice(&private_before).unwrap();
        assert_ne!(
            serde_json::json!(reopened.credential().credential_id),
            original["receipt"]["credential"]["credential_id"]
        );
        assert_eq!(
            fs::read(f.client_control.root.join("body/native-part-private.json")).unwrap(),
            private_before
        );
        verify_offline(&f);
    }
}

#[test]
fn changed_original_route_pin_and_expired_selection_refuse_before_connect() {
    let f = fixture();
    let mut changed = f.route.clone();
    changed.authentication.transport_binding_sha256 = [99; 32];
    let verified = control::native_snapshot::start(
        &f.owner.root,
        "host/snapshot-client",
        10_000,
        &f.selected,
        &changed,
    )
    .unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;
    let client = f.client.as_ref().unwrap();
    assert!(client
        .validate_selection(&verified, now)
        .unwrap_err()
        .contains("retained route mismatch"));
    browser::cancel(&f.owner.root, &verified.authorization().window_id).unwrap();
    let verified = control::native_snapshot::start(
        &f.owner.root,
        "host/snapshot-client",
        10_000,
        &f.selected,
        &f.route,
    )
    .unwrap();
    assert!(client
        .validate_selection(&verified, f.route.expires_at_millis)
        .unwrap_err()
        .contains("expired"));
    browser::cancel(&f.owner.root, &verified.authorization().window_id).unwrap();
    verify_offline(&f);
}
