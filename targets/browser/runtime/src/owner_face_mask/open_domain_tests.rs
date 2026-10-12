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
    let reference = serde_json::json!({
        "schema":"conduit.proof/face-mask-reference@1",
        "proof_class":"checked browser Mask with simulated DOM acknowledgement",
        "face_file":"open-domain-face.json",
        "request_summary":{
            "input_fore":"face", "output_fore":"show", "return_fore":"interaction",
            "face_id":face.identity.as_str(), "face_revision":face.revision,
            "interaction_context":face.interaction_context, "source_basis":face.basis,
            "mask_plot_id":prepared.mask_plot_id, "mask_expanded_plot_id":prepared.mask_expanded_plot_id,
            "mask_plan_id":prepared.mask_plan_id, "mask_play_id":prepared.mask_play_id,
            "host_id":"host/browser-open-domain-fixture", "boot_id":"boot/browser-open-domain-fixture",
            "show_id":prepared.show_id,
        },
        "returned_interaction":emitted.interaction,
        "source_effect_applied":false,
    });
    let recorded:serde_json::Value=serde_json::from_str(include_str!(
        "../../../../../proof/conformance/presentation-waist/domain-derived/face-mask-reference.json"
    )).unwrap();
    assert_eq!(
        recorded, reference,
        "reference IDs differ from actual checked Mask execution"
    );
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

#[test]
fn browser_generic_mask_keeps_one_body_lifecycle_and_rejects_the_other_states_revision() {
    let lulled: Presentation = serde_json::from_str(include_str!(
        "../../../../../proof/conformance/presentation-waist/domain-derived/lulled-body-face.json"
    ))
    .unwrap();
    let awake: Presentation = serde_json::from_str(include_str!(
        "../../../../../proof/conformance/presentation-waist/domain-derived/awake-body-face.json"
    ))
    .unwrap();
    assert_eq!(lulled.basis.body_id, awake.basis.body_id);
    for (index, (face, other)) in [(&lulled, &awake), (&awake, &lulled)]
        .into_iter()
        .enumerate()
    {
        let mut mask = OwnerBrowserMask::prepare_component_fixture(
            HostBasis {
                visual_constraints: None,
                body_id: face.basis.body_id.clone().unwrap(),
                host_id: HostId::from("host/browser-lifecycle-fixture"),
                boot_id: BootId::from("boot/browser-lifecycle-fixture"),
            },
            face.clone(),
            index as u64 + 1,
            true,
        )
        .unwrap();
        let view = mask.view();
        assert_eq!(view.face_id, face.identity.as_str());
        assert_eq!(view.face_revision, face.revision.to_string());
        mask.acknowledge(Acknowledgement {
            show_id: view.show_id.clone(),
            face_id: view.face_id.clone(),
            face_revision: view.face_revision.clone(),
        })
        .unwrap();
        let action = face
            .actions
            .iter()
            .find(|action| action.availability.is_available() && action.arguments.is_empty())
            .unwrap();
        let proposal = |revision: String| ProposedInteraction {
            show_id: view.show_id.clone(),
            face_id: view.face_id.clone(),
            face_revision: revision,
            action_id: action.identity.clone(),
            target: action.target.clone(),
            arguments: vec![],
            sequence: 1,
        };
        assert!(mask.interact(proposal(other.revision.to_string())).is_err());
        let emitted = mask.interact(proposal(view.face_revision.clone())).unwrap();
        assert_eq!(emitted.interaction.face_id, face.identity.as_str());
        assert_eq!(emitted.interaction.action_id, action.identity);
        assert_eq!(mask.presentation, *face);
        assert!(mask.presentation.basis.plan_id.is_none());
        assert!(mask.presentation.basis.active_play_id.is_none());
    }
}

#[test]
fn browser_mask_refuses_pressure_and_lost_shows_before_typed_emission() {
    let face: Presentation = serde_json::from_str(include_str!(
        "../../../../../proof/conformance/presentation-waist/domain-derived/open-domain-face.json"
    ))
    .unwrap();
    for (index, lifecycle) in [
        ManifestationLifecycle::Closed,
        ManifestationLifecycle::Failed,
        ManifestationLifecycle::Replaced,
    ]
    .into_iter()
    .enumerate()
    {
        let mut mask = OwnerBrowserMask::prepare_component_fixture(
            HostBasis {
                visual_constraints: None,
                body_id: face.basis.body_id.clone().unwrap(),
                host_id: HostId::from("host/browser-loss-fixture"),
                boot_id: BootId::from("boot/browser-loss-fixture"),
            },
            face.clone(),
            index as u64 + 1,
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
        let proposed = |value: String| ProposedInteraction {
            show_id: prepared.show_id.clone(),
            face_id: prepared.face_id.clone(),
            face_revision: prepared.face_revision.clone(),
            action_id: "lesson/answer".into(),
            target: "concept/mitochondrion".into(),
            arguments: vec![ProposedArgument {
                name: "lesson/answer-text".into(),
                value,
            }],
            sequence: 1,
        };
        assert!(mask.interact(proposed("x".repeat(129))).is_err());
        assert_eq!(mask.presentation, face);
        assert_eq!(mask.view().show_state, "available");
        mask.show = if lifecycle == ManifestationLifecycle::Failed {
            mask.show
                .fail(
                    conduit_presentation::ManifestationFailure::AdapterUnavailable,
                    SignId::from("sign/browser-domain-adapter-lost"),
                )
                .unwrap()
        } else {
            mask.show
                .transition(lifecycle, SignId::from("sign/browser-domain-show-closed"))
                .unwrap()
        };
        assert_eq!(
            mask.view().show_state,
            match lifecycle {
                ManifestationLifecycle::Closed => "closed",
                ManifestationLifecycle::Failed => "failed",
                ManifestationLifecycle::Replaced => "replaced",
                _ => unreachable!(),
            }
        );
        assert!(mask.interact(proposed("Energy".into())).is_err());
        assert_eq!(mask.presentation, face);
    }
}
