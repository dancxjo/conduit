//! Domain-derived Show cancellation/loss across the Body-backed shared corpus.
//! These are checked Mask tests with simulated acknowledgement, not DOM effects.
use super::*;

#[test]
fn domain_corpus_terminal_shows_refuse_before_interaction_emission() {
    let specimens = [
        include_str!("../../../../../proof/conformance/presentation-waist/domain-derived/open-domain-face.json"),
        include_str!("../../../../../proof/conformance/presentation-waist/domain-derived/finite-place-face.json"),
        include_str!("../../../../../proof/conformance/presentation-waist/domain-derived/lulled-body-face.json"),
        include_str!("../../../../../proof/conformance/presentation-waist/domain-derived/awake-body-face.json"),
        include_str!("../../../../../proof/conformance/presentation-waist/domain-derived/patchbay-face.json"),
    ];
    for specimen in specimens {
        let face: Presentation = serde_json::from_str(specimen).unwrap();
        face.validate().unwrap();
        for lifecycle in [
            ManifestationLifecycle::Closed,
            ManifestationLifecycle::Replaced,
            ManifestationLifecycle::Failed,
        ] {
            let mut mask = OwnerBrowserMask::prepare_component_fixture(
                HostBasis {
                    visual_constraints: None,
                    body_id: face.basis.body_id.clone().unwrap(),
                    host_id: HostId::from("host/browser-corpus-loss-fixture"),
                    boot_id: BootId::from("boot/browser-corpus-loss-fixture"),
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
            assert!(mask.pending_interaction.is_some());
            mask.show = if lifecycle == ManifestationLifecycle::Failed {
                mask.show
                    .fail(
                        conduit_presentation::ManifestationFailure::AdapterUnavailable,
                        SignId::from("sign/corpus-adapter-loss"),
                    )
                    .unwrap()
            } else {
                mask.show
                    .transition(lifecycle, SignId::from("sign/corpus-show-retired"))
                    .unwrap()
            };
            let action = face
                .actions
                .iter()
                .find(|action| action.availability.is_available())
                .unwrap();
            let proposed = ProposedInteraction {
                show_id: prepared.show_id,
                face_id: prepared.face_id,
                face_revision: prepared.face_revision,
                action_id: action.identity.clone(),
                target: action.target.clone(),
                arguments: action
                    .arguments
                    .iter()
                    .map(|argument| ProposedArgument {
                        name: argument.name.clone(),
                        value: "Energy".into(),
                    })
                    .collect(),
                sequence: 1,
            };
            match mask.interact(proposed) {
                Err(reason) => assert_eq!(
                    reason,
                    "owner interaction return is not admitted for this Show"
                ),
                Ok(_) => panic!("retired Show emitted interaction for {:?}", face.identity),
            }
            assert!(
                mask.pending_interaction.is_some(),
                "refusal consumed interaction request"
            );
            assert_eq!(mask.presentation, face);
            assert!(mask
                .scheduler
                .remote_egress_offer(
                    mask.interaction_boundary.endpoint,
                    mask.interaction_boundary.cord
                )
                .unwrap()
                .is_none());
        }
    }
}
