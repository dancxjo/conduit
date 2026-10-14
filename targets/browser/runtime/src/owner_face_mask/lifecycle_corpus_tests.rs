//! Domain-derived Show cancellation/loss across the Body-backed shared corpus.
//! These are checked Mask tests with simulated acknowledgement, not DOM effects.
use super::*;

#[test]
fn domain_corpus_terminal_shows_and_cancelled_execution_refuse_before_interaction_emission() {
    let specimens = [
        include_str!(
            "../../../../../proof/conformance/face-waist/domain-derived/open-domain-face.json"
        ),
        include_str!(
            "../../../../../proof/conformance/face-waist/domain-derived/finite-place-face.json"
        ),
        include_str!(
            "../../../../../proof/conformance/face-waist/domain-derived/lulled-body-face.json"
        ),
        include_str!(
            "../../../../../proof/conformance/face-waist/domain-derived/awake-body-face.json"
        ),
        include_str!(
            "../../../../../proof/conformance/face-waist/domain-derived/patchbay-face.json"
        ),
    ];
    for specimen in specimens {
        let face: Face = serde_json::from_str(specimen).unwrap();
        face.validate().unwrap();
        for lifecycle in [
            Some(ManifestationLifecycle::Closed),
            Some(ManifestationLifecycle::Replaced),
            Some(ManifestationLifecycle::Failed),
            None, // actual scheduler cancellation with an acknowledged Show
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
            let pending = mask.pending_interaction;
            assert!(pending.is_some());
            match lifecycle {
                None => mask.scheduler.cancel().unwrap(),
                Some(state) => {
                    mask.show = if state == ManifestationLifecycle::Failed {
                        mask.show
                            .fail(
                                conduit_face::ManifestationFailure::AdapterUnavailable,
                                SignId::from("sign/corpus-adapter-loss"),
                            )
                            .unwrap()
                    } else {
                        mask.show
                            .transition(state, SignId::from("sign/corpus-show-retired"))
                            .unwrap()
                    };
                }
            }
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
                Err(reason) => {
                    if lifecycle.is_none() {
                        assert_eq!(reason, "store owner browser interaction: Cancelled");
                    } else {
                        assert_eq!(
                            reason,
                            "owner interaction return is not admitted for this Show"
                        );
                    }
                }
                Ok(_) => panic!("retired Show emitted interaction for {:?}", face.identity),
            }
            assert_eq!(
                mask.pending_interaction, pending,
                "refusal changed interaction request"
            );
            assert_eq!(mask.face, face);
            let egress = mask.scheduler.remote_egress_offer(
                mask.interaction_boundary.endpoint,
                mask.interaction_boundary.cord,
            );
            if lifecycle.is_none() {
                assert!(matches!(
                    egress,
                    Err(conduit_kernel::scheduler::SchedulerError::Cancelled)
                ));
                assert!(mask.scheduler.next_host_request().is_none());
            } else {
                assert!(egress.unwrap().is_none());
            }
        }
    }
}
