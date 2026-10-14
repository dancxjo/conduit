//! Checked browser Mask over the source-derived Patchbay fixture.
//! Simulated Show acknowledgement; no installed owner effects are claimed.
use super::*;

#[test]
fn browser_mask_preserves_complete_source_derived_patchbay_face() {
    let face: Face = serde_json::from_str(include_str!(
        "../../../../../proof/conformance/face-waist/domain-derived/patchbay-face.json"
    ))
    .unwrap();
    face.validate().unwrap();
    let mut mask = OwnerBrowserMask::prepare_component_fixture(
        HostBasis {
            visual_constraints: None,
            body_id: face.basis.body_id.clone().unwrap(),
            host_id: HostId::from("host/browser-patchbay-fixture"),
            boot_id: BootId::from("boot/browser-patchbay-fixture"),
        },
        face.clone(),
        1,
        true,
    )
    .unwrap();
    let prepared = mask.view();
    assert_eq!(prepared.subjects.len(), face.subjects.len());
    for subject in &face.subjects {
        let rendered = prepared
            .subjects
            .iter()
            .find(|item| item.identity == subject.identity)
            .unwrap();
        assert_eq!(rendered.name, subject.name);
        assert_eq!(rendered.role, format!("{:?}", subject.role));
    }
    assert_eq!(prepared.relationships.len(), face.relationships.len());
    for (rendered, source) in prepared.relationships.iter().zip(&face.relationships) {
        assert_eq!(rendered.source, source.source);
        assert_eq!(rendered.target, source.target);
        assert_eq!(rendered.kind, format!("{:?}", source.kind));
    }
    mask.acknowledge(Acknowledgement {
        show_id: prepared.show_id.clone(),
        face_id: prepared.face_id.clone(),
        face_revision: prepared.face_revision.clone(),
    })
    .unwrap();
    assert_eq!(mask.view().show_state, "available");
    assert_eq!(mask.face, face);
}

#[test]
fn browser_patchbay_returns_only_supplied_available_actions() {
    let face: Face = serde_json::from_str(include_str!(
        "../../../../../proof/conformance/face-waist/domain-derived/patchbay-face.json"
    ))
    .unwrap();
    for action in &face.actions {
        assert!(action.arguments.is_empty());
        let mut mask = OwnerBrowserMask::prepare_component_fixture(
            HostBasis {
                visual_constraints: None,
                body_id: face.basis.body_id.clone().unwrap(),
                host_id: HostId::from("host/browser-patchbay-returns-fixture"),
                boot_id: BootId::from("boot/browser-patchbay-returns-fixture"),
            },
            face.clone(),
            1,
            true,
        )
        .unwrap();
        let prepared = mask.view();
        mask.acknowledge(Acknowledgement {
            show_id: prepared.show_id.clone(),
            face_id: prepared.face_id.clone(),
            face_revision: prepared.face_revision.clone(),
        })
        .unwrap();
        let proposed = |action_id: String, target: String, revision: String| ProposedInteraction {
            show_id: prepared.show_id.clone(),
            face_id: prepared.face_id.clone(),
            face_revision: revision,
            action_id,
            target,
            arguments: vec![],
            sequence: 1,
        };
        assert!(mask
            .interact(proposed(
                action.identity.clone(),
                action.target.clone(),
                (face.revision + 1).to_string()
            ))
            .is_err());
        assert!(mask
            .interact(proposed(
                "invented/patchbay-action".into(),
                action.target.clone(),
                prepared.face_revision.clone()
            ))
            .is_err());
        assert!(mask
            .interact(proposed(
                action.identity.clone(),
                "invented/target".into(),
                prepared.face_revision.clone()
            ))
            .is_err());
        let result = mask.interact(proposed(
            action.identity.clone(),
            action.target.clone(),
            prepared.face_revision.clone(),
        ));
        if action.availability.is_available() {
            let emitted = result.unwrap();
            assert_eq!(emitted.interaction.action_id, action.identity);
            assert_eq!(emitted.interaction.face_id, face.identity.as_str());
            assert_eq!(emitted.interaction.face_revision, face.revision);
            assert_eq!(emitted.show.show_id.as_str(), prepared.show_id);
        } else {
            assert!(
                result.is_err(),
                "unavailable action emitted: {}",
                action.identity
            );
        }
        assert_eq!(mask.face, face);
    }
}
