//! Explicit producer/consumer proof over freshly executed owner checkpoints.
//! Run the owner producer first with CONDUIT_CROSS_MASK_TODO_EXCHANGE set.
use super::*;

#[test]
#[ignore = "requires a freshly executed owner checkpoint exchange"]
fn checked_browser_mask_consumes_actual_committed_todo_exchange() {
    let path = std::env::var_os("CONDUIT_CROSS_MASK_TODO_EXCHANGE").expect("owner exchange path");
    let recorded: serde_json::Value =
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(
        recorded["schema"],
        "conduit.proof/committed-todo-mask-exchange@1"
    );
    let exchanges = recorded["exchanges"].as_array().unwrap();
    assert_eq!(exchanges.len(), 2);
    let mut body = None;
    for (index, exchange) in exchanges.iter().enumerate() {
        let face: Face = serde_json::from_value(exchange["face"].clone()).unwrap();
        face.validate().unwrap();
        let body_id = face.basis.body_id.clone().unwrap();
        if let Some(previous) = &body {
            assert_eq!(previous, &body_id);
        }
        body = Some(body_id.clone());
        let receipt = &exchange["verified_read_receipt"];
        assert_eq!(receipt["body_id"], body_id.as_str());
        assert_eq!(receipt["read_terminal"], "Completed");
        assert_eq!(receipt["write"]["terminal"], "Completed");
        for sign in [
            &receipt["read_terminal_sign"]["sign_id"],
            &receipt["write"]["terminal_sign"]["sign_id"],
        ] {
            let sign = sign.as_str().unwrap();
            assert!(face.basis.sign_ids.iter().any(|id| id.as_str() == sign));
        }
        let mut mask = OwnerBrowserMask::prepare_component_fixture(
            HostBasis {
                visual_constraints: None,
                body_id,
                host_id: HostId::from("host/browser-committed-todo-fixture"),
                boot_id: BootId::from("boot/browser-committed-todo-fixture"),
            },
            face.clone(),
            index as u64 + 1,
            true,
        )
        .unwrap();
        let view = mask.view();
        assert!(matches_required_todo_meaning(&face, &view));
        let mut lost_status = mask.view();
        lost_status
            .subjects
            .retain(|subject| subject.role != "Status");
        assert!(!matches_required_todo_meaning(&face, &lost_status));
        let mut lost_status_text = mask.view();
        let status = lost_status_text
            .subjects
            .iter_mut()
            .find(|subject| subject.identity == "todo/status")
            .unwrap();
        assert!(!status.text.is_empty());
        status.text.clear();
        assert!(!matches_required_todo_meaning(&face, &lost_status_text));
        let mut wrong_endpoint = mask.view();
        wrong_endpoint.relationships[0].target = "invented/body-port".into();
        assert!(!matches_required_todo_meaning(&face, &wrong_endpoint));
        let mut invented_relation = mask.view();
        invented_relation.relationships.push(RelationshipView {
            source: "invented/port".into(),
            target: "invented/cord".into(),
            kind: "Connects".into(),
        });
        assert!(!matches_required_todo_meaning(&face, &invented_relation));
        let items = view
            .subjects
            .iter()
            .filter(|subject| subject.role == "Item")
            .collect::<Vec<_>>();
        assert_eq!(items.len(), 3);
        let add = face
            .actions
            .iter()
            .find(|action| action.identity == "todo.add")
            .unwrap();
        let rendered_add = view
            .actions
            .iter()
            .find(|action| action.identity == add.identity)
            .unwrap();
        assert_eq!(rendered_add.arguments.len(), add.arguments.len());
        for (actual, source) in rendered_add.arguments.iter().zip(&add.arguments) {
            let minimum = source
                .contract
                .constraints
                .iter()
                .filter_map(|constraint| match constraint {
                    ValueConstraint::ByteLength { minimum, .. } => Some(*minimum),
                    _ => None,
                })
                .max()
                .unwrap_or(0);
            assert_eq!(actual.minimum_bytes, minimum);
            assert_eq!(actual.maximum_bytes, source.contract.maximum_bytes);
            assert!(minimum > 0, "the real Todo Add requires nonempty text");
        }
        for name in ["Milk", "Eggs", "Bread"] {
            assert!(items.iter().any(|item| item.name == name));
        }
        let remaining = items
            .iter()
            .filter(|item| {
                item.flags
                    .iter()
                    .any(|flag| flag.name == "complete" && !flag.value)
            })
            .count();
        assert_eq!(remaining, 3 - index);
        assert_eq!(exchange["remaining"], remaining);
        mask.acknowledge(Acknowledgement {
            show_id: view.show_id.clone(),
            face_id: view.face_id.clone(),
            face_revision: view.face_revision.clone(),
        })
        .unwrap();
        assert_eq!(mask.view().show_state, "available");
        let action = face
            .actions
            .iter()
            .find(|action| action.identity == "todo.complete.task-2")
            .unwrap();
        assert!(action.availability.is_available());
        let proposed = |show_id: String, revision: String, action_id: String, target: String| {
            ProposedInteraction {
                show_id,
                face_id: view.face_id.clone(),
                face_revision: revision,
                action_id,
                target,
                arguments: vec![],
                sequence: 1,
            }
        };
        assert!(mask
            .interact(proposed(
                "show/stale".into(),
                view.face_revision.clone(),
                action.identity.clone(),
                action.target.clone()
            ))
            .is_err());
        assert!(mask
            .interact(proposed(
                view.show_id.clone(),
                (face.revision + 1).to_string(),
                action.identity.clone(),
                action.target.clone()
            ))
            .is_err());
        assert!(mask
            .interact(proposed(
                view.show_id.clone(),
                view.face_revision.clone(),
                "todo.complete.invented".into(),
                action.target.clone()
            ))
            .is_err());
        let unavailable = face
            .actions
            .iter()
            .find(|action| !action.availability.is_available())
            .unwrap();
        assert!(mask
            .interact(proposed(
                view.show_id.clone(),
                view.face_revision.clone(),
                unavailable.identity.clone(),
                unavailable.target.clone()
            ))
            .is_err());
        let emitted = mask
            .interact(proposed(
                view.show_id.clone(),
                view.face_revision.clone(),
                action.identity.clone(),
                action.target.clone(),
            ))
            .unwrap();
        assert_eq!(emitted.interaction.face_id, face.identity.as_str());
        assert_eq!(emitted.interaction.face_revision, face.revision);
        assert_eq!(emitted.interaction.show_id, view.show_id);
        assert_eq!(emitted.interaction.action_id, action.identity);
        assert_eq!(emitted.interaction.target, action.target);
        // The returned interaction is not submitted to the owner in this proof.
        assert_eq!(mask.face, face);
        check_retired_todo_shows(&face);
    }
}

fn check_retired_todo_shows(face: &Face) {
    let add = face
        .actions
        .iter()
        .find(|action| action.identity == "todo.add")
        .unwrap();
    assert!(add.availability.is_available());
    assert_eq!(add.arguments.len(), 1);
    let argument = &add.arguments[0];
    for lifecycle in [
        ManifestationLifecycle::Closed,
        ManifestationLifecycle::Replaced,
        ManifestationLifecycle::Failed,
    ] {
        let mut mask = OwnerBrowserMask::prepare_component_fixture(
            HostBasis {
                visual_constraints: None,
                body_id: face.basis.body_id.clone().unwrap(),
                host_id: HostId::from("host/browser-committed-todo-loss-fixture"),
                boot_id: BootId::from("boot/browser-committed-todo-loss-fixture"),
            },
            face.clone(),
            1,
            true,
        )
        .unwrap();
        let view = mask.view();
        mask.acknowledge(Acknowledgement {
            show_id: view.show_id.clone(),
            face_id: view.face_id.clone(),
            face_revision: view.face_revision.clone(),
        })
        .unwrap();
        let proposed = |value: String| ProposedInteraction {
            show_id: view.show_id.clone(),
            face_id: view.face_id.clone(),
            face_revision: view.face_revision.clone(),
            action_id: add.identity.clone(),
            target: add.target.clone(),
            arguments: vec![ProposedArgument {
                name: argument.name.clone(),
                value,
            }],
            sequence: 1,
        };
        assert!(mask
            .interact(proposed(
                "x".repeat(argument.contract.maximum_bytes as usize + 1)
            ))
            .is_err());
        assert!(mask.pending_interaction.is_some());
        assert_eq!(mask.view().show_state, "available");
        assert!(mask
            .scheduler
            .remote_egress_offer(
                mask.interaction_boundary.endpoint,
                mask.interaction_boundary.cord
            )
            .unwrap()
            .is_none());
        mask.show = if lifecycle == ManifestationLifecycle::Failed {
            mask.show
                .fail(
                    conduit_face::ManifestationFailure::AdapterUnavailable,
                    SignId::from("sign/committed-todo-adapter-loss"),
                )
                .unwrap()
        } else {
            mask.show
                .transition(lifecycle, SignId::from("sign/committed-todo-retired"))
                .unwrap()
        };
        match mask.interact(proposed("Tea".into())) {
            Err(reason) => assert_eq!(
                reason,
                "owner interaction return is not admitted for this Show"
            ),
            Ok(_) => panic!("retired committed Todo Show emitted an interaction"),
        }
        assert!(mask.pending_interaction.is_some());
        assert!(mask
            .scheduler
            .remote_egress_offer(
                mask.interaction_boundary.endpoint,
                mask.interaction_boundary.cord
            )
            .unwrap()
            .is_none());
        assert_eq!(&mask.face, face);
    }
}

// Semantic reconstruction comparison, not renderer attestation or a domain reducer.
fn matches_required_todo_meaning(face: &Face, view: &FaceView) -> bool {
    if view.subjects.len() != face.subjects.len()
        || view.relationships.len() != face.relationships.len()
    {
        return false;
    }
    for source in &face.subjects {
        let Some(rendered) = view
            .subjects
            .iter()
            .find(|item| item.identity == source.identity)
        else {
            return false;
        };
        if rendered.name != source.name || rendered.role != format!("{:?}", source.role) {
            return false;
        }
        if source.role == conduit_face::FaceRole::Status {
            let expected: Vec<_> = face
                .text
                .iter()
                .filter(|text| text.subject == source.identity)
                .map(|text| text.text.clone())
                .collect();
            if rendered.text != expected {
                return false;
            }
        }
    }
    view.relationships
        .iter()
        .zip(&face.relationships)
        .all(|(rendered, source)| {
            rendered.source == source.source
                && rendered.target == source.target
                && rendered.kind == format!("{:?}", source.kind)
        })
}
