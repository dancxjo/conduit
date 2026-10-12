//! Browser Mask execution over the source-verified open-domain fixture.
//! Show acknowledgement is simulated here; no DOM or owner effect is claimed.
use super::*;

#[test]
fn browser_generic_mask_keeps_open_domain_ids_and_supplied_typed_agency() {
    let face: Presentation = serde_json::from_str(include_str!(
        "../../../../../proof/conformance/presentation-waist/domain-derived/open-domain-face.json"
    ))
    .unwrap();
    face.validate().unwrap();
    let basis = HostBasis {
        visual_constraints: None,
        body_id: face.basis.body_id.clone().unwrap(),
        host_id: HostId::from("host/browser-open-domain-fixture"),
        boot_id: BootId::from("boot/browser-open-domain-fixture"),
    };
    let mut mask =
        OwnerBrowserMask::prepare_component_fixture(basis, face.clone(), 1, true).unwrap();
    let prepared = mask.view();
    assert_eq!(prepared.face_id, face.identity.as_str());
    assert_eq!(prepared.face_revision, face.revision.to_string());
    let encoded = serde_json::to_string(&prepared).unwrap();
    for meaning in [
        "biology/cell/organelle",
        "biology/cell/contains-organelle",
        "Mitochondrion",
        "lesson/answer",
    ] {
        assert!(encoded.contains(meaning), "browser Mask lost {meaning}");
    }
    mask.acknowledge(Acknowledgement {
        show_id: prepared.show_id.clone(),
        face_id: prepared.face_id.clone(),
        face_revision: prepared.face_revision.clone(),
    })
    .unwrap();
    let proposed = |revision: String, action: &str| ProposedInteraction {
        show_id: prepared.show_id.clone(),
        face_id: prepared.face_id.clone(),
        face_revision: revision,
        action_id: action.into(),
        target: "concept/mitochondrion".into(),
        arguments: vec![ProposedArgument {
            name: "lesson/answer-text".into(),
            value: "Energy".into(),
        }],
        sequence: 1,
    };
    assert!(mask
        .interact(proposed((face.revision + 1).to_string(), "lesson/answer"))
        .is_err());
    assert!(mask
        .interact(proposed(prepared.face_revision.clone(), "invented/answer"))
        .is_err());
    let emitted = mask
        .interact(proposed(prepared.face_revision.clone(), "lesson/answer"))
        .unwrap();
    assert_eq!(emitted.interaction.action_id, "lesson/answer");
    assert_eq!(emitted.interaction.face_id, face.identity.as_str());
    assert_eq!(emitted.interaction.face_revision, face.revision);
    assert_eq!(emitted.interaction.arguments[0].value, b"Energy");
    assert_eq!(emitted.show.show_id.as_str(), prepared.show_id);
    assert_eq!(mask.presentation, face);
}

#[test]
fn browser_generic_mask_keeps_finite_place_actions_bound_to_their_targets() {
    let face: Presentation = serde_json::from_str(include_str!(
        "../../../../../proof/conformance/presentation-waist/domain-derived/finite-place-face.json"
    ))
    .unwrap();
    let mut mask = OwnerBrowserMask::prepare_component_fixture(
        HostBasis {
            visual_constraints: None,
            body_id: face.basis.body_id.clone().unwrap(),
            host_id: HostId::from("host/browser-finite-place-fixture"),
            boot_id: BootId::from("boot/browser-finite-place-fixture"),
        },
        face.clone(),
        1,
        true,
    )
    .unwrap();
    let prepared = mask.view();
    let encoded = serde_json::to_string(&prepared).unwrap();
    for meaning in [
        "Hub",
        "Workshop",
        "Crèche",
        "commons/place",
        "commons/contains-place",
    ] {
        assert!(encoded.contains(meaning), "lost {meaning}");
    }
    mask.acknowledge(Acknowledgement {
        show_id: prepared.show_id.clone(),
        face_id: prepared.face_id.clone(),
        face_revision: prepared.face_revision.clone(),
    })
    .unwrap();
    let proposal = |target: &str| ProposedInteraction {
        show_id: prepared.show_id.clone(),
        face_id: prepared.face_id.clone(),
        face_revision: prepared.face_revision.clone(),
        action_id: "commons/hub/inspect".into(),
        target: target.into(),
        arguments: vec![],
        sequence: 1,
    };
    assert!(mask.interact(proposal("commons/workshop")).is_err());
    let mut stale = proposal("commons/hub");
    stale.show_id.push_str("-retired");
    assert!(mask.interact(stale).is_err());
    let mut unavailable = proposal("commons/creche");
    unavailable.action_id = "commons/creche/inspect".into();
    assert!(mask.interact(unavailable).is_err());
    assert_eq!(mask.presentation, face);
    let emitted = mask.interact(proposal("commons/hub")).unwrap();
    assert_eq!(emitted.interaction.action_id, "commons/hub/inspect");
    assert_eq!(emitted.interaction.target, "commons/hub");
    assert!(emitted.interaction.arguments.is_empty());
    assert_eq!(emitted.show.show_id.as_str(), prepared.show_id);
    assert_eq!(mask.presentation, face);
}
