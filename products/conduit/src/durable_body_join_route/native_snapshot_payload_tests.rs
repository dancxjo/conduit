//! Authenticated transport does not excuse payloads contrary to the selected Body.
use super::*;
#[test]
fn real_protected_peer_wrong_schema_or_body_is_refused() {
    for wrong_schema in [false, true] {
        let mut f = fixture();
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
                    face.basis.body_id = Some(BodyId::from("body/not-selected"));
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
        verify_offline(&f);
    }
}
