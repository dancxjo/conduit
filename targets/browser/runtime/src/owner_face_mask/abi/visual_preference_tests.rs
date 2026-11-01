//! An available nondefault rendering cannot be reused to acknowledge its clear.
use super::*;
use crate::owner_face_mask::component::VisualConstraints;

#[test]
fn clearing_prepared_scale_or_platform_constraints_requires_full_snapshot() {
    let face: Face = serde_json::from_str(include_str!(
        "../../../../../../proof/conformance/face-waist/domain-derived/open-domain-face.json"
    ))
    .unwrap();
    for constraints in [
        VisualConstraints {
            minimum_text_scale_percent: 200,
            ..Default::default()
        },
        VisualConstraints {
            minimum_text_scale_percent: 100,
            forced_colors: true,
            ..Default::default()
        },
        VisualConstraints {
            minimum_text_scale_percent: 100,
            reduced_motion: true,
            ..Default::default()
        },
    ] {
        conduit_browser_owner_face_clear();
        let mut mask = OwnerBrowserMask::prepare_component_fixture(
            HostBasis {
                body_id: face.basis.body_id.clone().unwrap(),
                host_id: "host/constraint-cache".into(),
                boot_id: "boot/constraint-cache".into(),
                visual_constraints: Some(constraints),
            },
            face.clone(),
            1,
            false,
        )
        .unwrap();
        let view = mask.view();
        let show = view.show_id.clone();
        mask.acknowledge(Acknowledgement {
            show_id: view.show_id,
            face_id: view.face_id,
            face_revision: view.face_revision,
        })
        .unwrap();
        CURRENT.with(|current| *current.borrow_mut() = Some(mask));
        let basis = serde_json::json!({"body_id":face.basis.body_id,"host_id":"host/constraint-cache",
            "boot_id":"boot/constraint-cache","visual_constraints":{"minimum_text_scale_percent":100}});
        assert_eq!(
            super::tests::send(
                &basis,
                &OwnerFaceSnapshotResponse::Unchanged {
                    schema: OWNER_FACE_RESPONSE_SCHEMA.into(),
                    revision: face.revision,
                    identity: face.identity.clone(),
                }
            ),
            0
        );
        let refused: serde_json::Value =
            OUTPUT.with(|output| serde_json::from_slice(&output.borrow()).unwrap());
        assert_eq!(
            refused["code"],
            "owner-visual-preferences-require-full-snapshot"
        );
        assert_eq!(conduit_browser_owner_face_canonical_snapshot(), 0);
        let snapshot: serde_json::Value =
            OUTPUT.with(|output| serde_json::from_slice(&output.borrow()).unwrap());
        let retained: Face = serde_json::from_value(snapshot["face"].clone()).unwrap();
        assert_eq!(retained, face);
        assert_eq!(snapshot["show_id"], show);
        conduit_browser_owner_face_clear();
    }
}
