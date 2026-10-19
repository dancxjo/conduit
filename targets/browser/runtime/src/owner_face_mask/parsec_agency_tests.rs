//! Checked browser Fore proof over validated captured Parsec Faces.
//! This fixture acknowledges its own Show; captured terminal custody is not reused.
use super::*;

#[test]
#[ignore = "requires original-frame validated records in CONDUIT_PARSEC_BROWSER_RECORDS"]
fn checked_browser_mask_returns_exact_captured_parsec_actions() {
    let path = std::path::PathBuf::from(
        std::env::var_os("CONDUIT_PARSEC_BROWSER_RECORDS").expect("validated capture records"),
    );
    assert!(path.is_absolute());
    let records: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(
        records["schema"],
        "conduit.proof/parsec-component-reconstruction@1"
    );
    let records = records["records"].as_array().unwrap();
    assert_eq!(records.len(), 4);
    let mut original_body = None;
    for (index, record) in records.iter().enumerate() {
        let face: Face = serde_json::from_value(record["face"].clone()).unwrap();
        face.validate().unwrap();
        let body = face.basis.body_id.clone().unwrap();
        if let Some(previous) = &original_body {
            assert_eq!(previous, &body);
        }
        original_body = Some(body.clone());
        assert_eq!(record["provenance"]["body_play"]["body_id"], body.as_str());
        assert_eq!(
            record["provenance"]["body_play"]["active_play_id"],
            face.basis.active_play_id.as_ref().unwrap().as_str()
        );
        let action = &face.actions[0];
        assert_eq!(record["provenance"]["action"], action.identity);
        assert_eq!(record["face_revision_decimal"], face.revision.to_string());
        assert_eq!(
            action.identity,
            if index == 0 {
                "game/start"
            } else {
                "game/shoot"
            }
        );
        assert!(action.availability.is_available());
        let basis = || HostBasis {
            visual_constraints: None,
            body_id: face.basis.body_id.clone().unwrap(),
            host_id: HostId::from("host/browser-parsec-fore-fixture"),
            boot_id: BootId::from("boot/browser-parsec-fore-fixture"),
        };
        let mut mask = OwnerBrowserMask::prepare_component_fixture(
            basis(),
            face.clone(),
            index as u64 + 1,
            true,
        )
        .unwrap();
        let view = mask.view();
        assert_eq!(view.body_id.as_deref(), Some(body.as_str()));
        assert_eq!(view.face_id, face.identity.as_str());
        assert_eq!(view.face_revision, face.revision.to_string());
        assert_ne!(view.show_id, record["provenance"]["source_show"]);
        let proposal = || ProposedInteraction {
            show_id: view.show_id.clone(),
            face_id: view.face_id.clone(),
            face_revision: view.face_revision.clone(),
            action_id: action.identity.clone(),
            target: action.target.clone(),
            arguments: vec![],
            sequence: 1,
        };
        assert!(
            mask.interact(proposal()).is_err(),
            "prepared Show has no agency"
        );
        mask.acknowledge(Acknowledgement {
            show_id: view.show_id.clone(),
            face_id: view.face_id.clone(),
            face_revision: view.face_revision.clone(),
        })
        .unwrap();
        let pending = mask.pending_interaction;
        assert!(pending.is_some());
        for field in ["show", "revision", "action", "target", "argument"] {
            let mut changed = proposal();
            match field {
                "show" => changed.show_id = "show/stale".into(),
                "revision" => changed.face_revision = (face.revision + 1).to_string(),
                "action" => changed.action_id = "game/invented".into(),
                "target" => changed.target = "invented/body".into(),
                _ => changed.arguments.push(ProposedArgument {
                    name: "invented".into(),
                    value: "mutation".into(),
                }),
            }
            assert!(
                mask.interact(changed).is_err(),
                "{field} refuses before Fore emission"
            );
            assert_eq!(mask.pending_interaction, pending);
            assert!(mask
                .scheduler
                .remote_egress_offer(
                    mask.interaction_boundary.endpoint,
                    mask.interaction_boundary.cord
                )
                .unwrap()
                .is_none());
        }
        let emitted = mask.interact(proposal()).unwrap();
        assert_eq!(emitted.interaction.face_id, face.identity.as_str());
        assert_eq!(emitted.interaction.face_revision, face.revision);
        assert_eq!(emitted.interaction.action_id, action.identity);
        assert_eq!(emitted.interaction.target, action.target);
        assert_eq!(emitted.interaction.show_id, view.show_id);
        assert!(emitted.interaction.arguments.is_empty());
        emitted.show.validate(&face).unwrap();
        assert_eq!(emitted.show.show.active_play_id.as_str(), view.mask_play_id);
        assert_eq!(emitted.show.show.play_sequence, index as u64 + 1);
        assert_eq!(emitted.show.show.body_id.as_ref(), Some(&body));
        assert_eq!(
            emitted.show.planned_mask.plan.plan_id.as_str(),
            view.mask_plan_id
        );
        let placement = emitted.show.planned_mask.show_placement();
        assert_eq!(
            placement.host_id.as_str(),
            "host/browser-parsec-fore-fixture"
        );
        assert_eq!(
            placement.boot_id.as_str(),
            "boot/browser-parsec-fore-fixture"
        );
        assert_ne!(
            Some(&emitted.show.planned_mask.plan.plan_id),
            face.basis.plan_id.as_ref()
        );
        assert_eq!(mask.face.interaction_context, face.interaction_context);
        assert_eq!(mask.face, face);
        let mut unavailable = face.clone();
        unavailable.actions[0].availability = conduit_face::FaceActionAvailability::Unavailable {
            reason_code: "fixture/input-unavailable".into(),
            explanation: "No current input in this fixture".into(),
        };
        let unavailable = unavailable.with_basis(face.basis.clone()).unwrap();
        unavailable.validate().unwrap();
        assert_ne!(unavailable.identity, face.identity);
        let mut unavailable_mask = OwnerBrowserMask::prepare_component_fixture(
            basis(),
            unavailable,
            index as u64 + 11,
            true,
        )
        .unwrap();
        let unavailable_view = unavailable_mask.view();
        unavailable_mask
            .acknowledge(Acknowledgement {
                show_id: unavailable_view.show_id.clone(),
                face_id: unavailable_view.face_id.clone(),
                face_revision: unavailable_view.face_revision.clone(),
            })
            .unwrap();
        let mut unavailable_proposal = proposal();
        unavailable_proposal.show_id = unavailable_view.show_id;
        unavailable_proposal.face_id = unavailable_view.face_id;
        unavailable_proposal.face_revision = unavailable_view.face_revision;
        let pending = unavailable_mask.pending_interaction;
        let refusal = match unavailable_mask.interact(unavailable_proposal) {
            Ok(_) => panic!("unavailable action emitted a return"),
            Err(refusal) => refusal,
        };
        assert!(refusal.contains("fixture/input-unavailable"), "{refusal}");
        assert_eq!(unavailable_mask.pending_interaction, pending);
        assert!(unavailable_mask
            .scheduler
            .remote_egress_offer(
                unavailable_mask.interaction_boundary.endpoint,
                unavailable_mask.interaction_boundary.cord
            )
            .unwrap()
            .is_none());
    }
}
