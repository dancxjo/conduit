use super::*;
use conduit_body::Body;
use conduit_core::{CheckedPlotId, SourceDocumentId};
use conduit_face::{BodyFace, FaceContext, FaceFocus};

pub(super) fn send(basis: &serde_json::Value, response: &OwnerFaceSnapshotResponse) -> i32 {
    let basis = serde_json::to_vec(basis).unwrap();
    let frame = serde_json::to_vec(&serde_json::json!({
        "kind": "face-snapshot-response", "protocol": 1, "response": response,
    }))
    .unwrap();
    assert!(basis.len() + frame.len() <= INPUT_CAPACITY);
    INPUT.with(|input| {
        let mut input = input.borrow_mut();
        input[..basis.len()].copy_from_slice(&basis);
        input[basis.len()..basis.len() + frame.len()].copy_from_slice(&frame);
    });
    conduit_browser_owner_face_prepare(basis.len(), frame.len())
}

#[test]
fn canonical_observation_requires_acknowledged_show_and_preserves_original_bytes() {
    conduit_browser_owner_face_clear();
    OUTPUT.with(|output| output.borrow_mut().extend_from_slice(b"stale"));
    assert_eq!(conduit_browser_owner_face_canonical_snapshot(), -6);
    assert_eq!(conduit_browser_owner_face_output_len(), 0);
    let body = Body::born(
        SourceDocumentId::from("source/canonical-observation"),
        CheckedPlotId::from("checked/canonical-observation"),
        1,
        SignId::from("sign/canonical-observation"),
    )
    .unwrap();
    let face = BodyFace::project(
        &body,
        None,
        u64::MAX,
        FaceContext::Overview,
        FaceFocus::Body,
        vec![],
    )
    .unwrap()
    .face;
    let mask = OwnerBrowserMask::prepare_component_fixture(
        HostBasis {
            body_id: body.body_id.clone(),
            host_id: HostId::from("host/canonical-observation"),
            boot_id: BootId::from("boot/canonical-observation"),
            visual_constraints: None,
        },
        face.clone(),
        1,
        false,
    )
    .unwrap();
    let prepared = mask.view();
    CURRENT.with(|current| *current.borrow_mut() = Some(mask));
    assert_eq!(conduit_browser_owner_face_canonical_snapshot(), -5);
    CURRENT.with(|current| {
        current
            .borrow_mut()
            .as_mut()
            .unwrap()
            .acknowledge(Acknowledgement {
                show_id: prepared.show_id.clone(),
                face_id: prepared.face_id,
                face_revision: prepared.face_revision,
            })
            .unwrap()
    });
    assert_eq!(conduit_browser_owner_face_canonical_snapshot(), 0);
    let bytes = OUTPUT.with(|output| output.borrow().clone());
    assert!(std::str::from_utf8(&bytes)
        .unwrap()
        .contains("18446744073709551615"));
    let observed: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let retained: Face = serde_json::from_value(observed["face"].clone()).unwrap();
    assert_eq!(retained, face);
    assert_eq!(observed["show_id"], prepared.show_id);
    assert_eq!(conduit_browser_owner_face_canonical_snapshot(), 0);
    assert_eq!(OUTPUT.with(|output| output.borrow().clone()), bytes);
    // Conditional reuse cannot claim that new Host-local preferences were
    // applied. The old exact Face/Show remains observable, not restyled.
    for constraints in [
        serde_json::json!({"minimum_text_scale_percent":100,"enhanced_contrast":true}),
        serde_json::json!({"minimum_text_scale_percent":200}),
        serde_json::json!({"minimum_text_scale_percent":100,"forced_colors":true}),
        serde_json::json!({"minimum_text_scale_percent":100,"reduced_motion":true}),
    ] {
        let basis = serde_json::json!({
            "body_id": body.body_id, "host_id": "host/canonical-observation",
            "boot_id": "boot/canonical-observation",
            "visual_constraints": constraints,
        });
        assert_eq!(
            send(
                &basis,
                &OwnerFaceSnapshotResponse::Unchanged {
                    schema: OWNER_FACE_RESPONSE_SCHEMA.into(),
                    revision: face.revision,
                    identity: face.identity.clone(),
                }
            ),
            0
        );
        let refusal: serde_json::Value =
            OUTPUT.with(|output| serde_json::from_slice(&output.borrow()).unwrap());
        assert_eq!(refusal["schema"], "conduit.browser/owner-face-refusal@1");
        assert_eq!(
            refusal["code"],
            "owner-visual-preferences-require-full-snapshot"
        );
        assert_eq!(conduit_browser_owner_face_canonical_snapshot(), 0);
        assert_eq!(OUTPUT.with(|output| output.borrow().clone()), bytes);
    }
    conduit_browser_owner_face_clear();
}

#[test]
fn raw_owner_face_frame_refuses_a_snapshot_without_an_owner_route() {
    conduit_browser_owner_face_clear();
    let body = Body::born(
        SourceDocumentId::from("source/browser-owner-abi"),
        CheckedPlotId::from("checked/browser-owner-abi"),
        1,
        SignId::from("sign/browser-owner-abi-birth"),
    )
    .unwrap();
    let revision = 9_007_199_254_740_993;
    let face = BodyFace::project(
        &body,
        None,
        revision,
        FaceContext::Overview,
        FaceFocus::Body,
        vec![],
    )
    .unwrap()
    .face;
    let basis = serde_json::json!({
        "body_id": body.body_id,
        "host_id": "host/browser-owner-abi",
        "boot_id": "boot/browser-owner-abi",
    });
    assert_eq!(
        send(
            &basis,
            &OwnerFaceSnapshotResponse::Snapshot {
                schema: OWNER_FACE_RESPONSE_SCHEMA.into(),
                face: Box::new(face.clone()),
                interactions_admitted: false,
                route: None,
            }
        ),
        -5
    );
    assert_eq!(
        send(
            &basis,
            &OwnerFaceSnapshotResponse::Unchanged {
                schema: OWNER_FACE_RESPONSE_SCHEMA.into(),
                revision,
                identity: face.identity,
            }
        ),
        -6
    );
    conduit_browser_owner_face_clear();
}
