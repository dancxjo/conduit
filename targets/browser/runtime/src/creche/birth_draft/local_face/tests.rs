//! Actual local owner mutation through Mask Fore execution, not Chromium proof.
use super::*;
use serde_json::json;
fn current() -> CurrentDraft {
    let mut current = CurrentDraft::default();
    current
        .open(OpenRequest {
            persona_uuid: "11111111-2222-4333-8444-555555555555".into(),
            choices: vec![Choice {
                title: "Clock".into(),
                search_text: "clock".into(),
                plot: ResidentPlot {
                    source_document_id: "source/clock".into(),
                    checked_plot_id: "checked/clock".into(),
                },
                selected: false,
            }],
        })
        .unwrap();
    current
}
fn prepare(current: &mut CurrentDraft, component: bool) -> serde_json::Value {
    current
        .prepare_local_face(Prepare {
            generation: current.generation,
            host_id: "host/local-creche".into(),
            boot_id: "boot/local-creche".into(),
            component,
            component_admission: component
                .then(crate::owner_face_mask::local_component::expected_admission),
        })
        .unwrap();
    current.local_face.as_ref().unwrap().snapshot()
}
fn acknowledge(current: &mut CurrentDraft, snapshot: &serde_json::Value) {
    let view = &snapshot["view"];
    current
        .acknowledge_local_face(Acknowledge {
            generation: current.generation,
            show_id: view["show_id"].as_str().unwrap().into(),
            face_id: view["face_id"].as_str().unwrap().into(),
            face_revision: view["face_revision"].as_str().unwrap().into(),
        })
        .unwrap();
}
fn proposal(snapshot: &serde_json::Value, value: &str) -> serde_json::Value {
    let action = snapshot["face"]["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["identity"] == "creche.name")
        .unwrap();
    json!({"show_id":snapshot["view"]["show_id"], "face_id":snapshot["view"]["face_id"],
        "face_revision":snapshot["view"]["face_revision"], "action_id":"creche.name", "target":action["target"],
        "arguments":[{"name":"value","value_kind":conduit_face::UTF8_TEXT_VALUE_KIND,"value":value.as_bytes()}],"sequence":1})
}
#[test]
fn generic_and_configured_local_masks_commit_the_actual_draft_once_without_body() {
    for component in [false, true] {
        let mut current = current();
        let snapshot = prepare(&mut current, component);
        assert!(snapshot["face"]["basis"]["body_id"].is_null());
        assert!(snapshot["view"]["body_id"].is_null());
        let bindings = &snapshot["control_bindings"];
        assert_eq!(bindings["face_identity"], snapshot["face"]["identity"]);
        assert_eq!(bindings["face_revision"], snapshot["view"]["face_revision"]);
        assert_eq!(bindings["unsupported"], json!([]));
        assert_eq!(
            bindings["text_entries"].as_array().unwrap().len(),
            snapshot["face"]["subjects"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|subject| subject["role"] == "TextEntry")
                .count()
        );
        let proposal = proposal(&snapshot, "東京 é");
        assert!(current
            .submit_local_face(Submit {
                generation: current.generation,
                interaction: proposal.clone()
            })
            .is_err());
        acknowledge(&mut current, &snapshot);
        current
            .submit_local_face(Submit {
                generation: current.generation,
                interaction: proposal.clone(),
            })
            .unwrap();
        assert_eq!(current.draft.as_ref().unwrap().friendly_name(), "東京 é");
        let revision = current.draft.as_ref().unwrap().revision();
        assert!(current
            .submit_local_face(Submit {
                generation: current.generation,
                interaction: proposal
            })
            .is_err());
        assert_eq!(current.draft.as_ref().unwrap().revision(), revision);
    }
}
#[test]
fn forged_stale_replaced_and_cancelled_encounters_have_no_owner_effect() {
    let mut current = current();
    let snapshot = prepare(&mut current, false);
    acknowledge(&mut current, &snapshot);
    let before = current.draft.as_ref().unwrap().friendly_name().to_owned();
    let mut forged = proposal(&snapshot, "forged");
    forged["target"] = json!("invented/target");
    assert!(current
        .submit_local_face(Submit {
            generation: current.generation,
            interaction: forged
        })
        .is_err());
    assert_eq!(current.draft.as_ref().unwrap().friendly_name(), before);
    let stale = proposal(&snapshot, "old");
    let next = prepare(&mut current, false);
    acknowledge(&mut current, &next);
    assert!(current
        .submit_local_face(Submit {
            generation: current.generation,
            interaction: stale
        })
        .is_err());
    current.close_local_face().unwrap();
    assert!(current
        .submit_local_face(Submit {
            generation: current.generation,
            interaction: proposal(&next, "closed")
        })
        .is_err());
    assert_eq!(current.draft.as_ref().unwrap().friendly_name(), before);
}

#[test]
fn independent_boolean_bytes_mutate_the_same_owner_and_wrong_kind_has_no_effect() {
    let mut current = current();
    current
        .open(OpenRequest {
            persona_uuid: "11111111-2222-4333-8444-555555555555".into(),
            choices: vec![Choice {
                title: "Clock".into(),
                search_text: "clock".into(),
                plot: ResidentPlot {
                    source_document_id: "source/clock".into(),
                    checked_plot_id: "checked/clock".into(),
                },
                selected: false,
            }],
        })
        .unwrap();
    let snapshot = prepare(&mut current, true);
    acknowledge(&mut current, &snapshot);
    let action = snapshot["face"]["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["identity"] == "creche.plot.0")
        .unwrap();
    let mut input = json!({"show_id":snapshot["view"]["show_id"],"face_id":snapshot["view"]["face_id"],
        "face_revision":snapshot["view"]["face_revision"],"action_id":action["identity"],"target":action["target"],
        "arguments":[{"name":"value","value_kind":"value/text","value":[1]}],"sequence":1});
    assert!(current
        .submit_local_face(Submit {
            generation: current.generation,
            interaction: input.clone()
        })
        .is_err());
    assert!(!current.draft.as_ref().unwrap().choices()[0].selected);
    input["arguments"][0]["value_kind"] = json!("value/bool");
    current
        .submit_local_face(Submit {
            generation: current.generation,
            interaction: input,
        })
        .unwrap();
    assert!(current.draft.as_ref().unwrap().choices()[0].selected);
    let next = prepare(&mut current, true);
    acknowledge(&mut current, &next);
    let action = next["face"]["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["identity"] == "creche.plot.0")
        .unwrap();
    let input = json!({"show_id":next["view"]["show_id"],"face_id":next["view"]["face_id"],
        "face_revision":next["view"]["face_revision"],"action_id":action["identity"],"target":action["target"],
        "arguments":[{"name":"value","value_kind":"value/bool","value":[0]}],"sequence":1});
    current
        .submit_local_face(Submit {
            generation: current.generation,
            interaction: input,
        })
        .unwrap();
    assert!(!current.draft.as_ref().unwrap().choices()[0].selected);
}

#[test]
fn finite_local_source_specimen_fits_the_admitted_transport_envelope() {
    let mut current = current();
    current
        .open(OpenRequest {
            persona_uuid: "11111111-2222-4333-8444-555555555555".into(),
            choices: (0..16)
                .map(|index| Choice {
                    title: format!("{}!", "界".repeat(21)),
                    search_text: "finite source specimen".into(),
                    plot: ResidentPlot {
                        source_document_id: format!("{index:064x}").into(),
                        checked_plot_id: format!("{index:064x}").into(),
                    },
                    selected: true,
                })
                .collect(),
        })
        .unwrap();
    current
        .draft
        .as_mut()
        .unwrap()
        .edit_name(1, "é".repeat(32))
        .unwrap();
    for component in [false, true] {
        let snapshot = prepare(&mut current, component);
        let bytes = serde_json::to_vec(&snapshot).unwrap().len();
        assert!(bytes < transport::conduit_creche_local_face_output_capacity());
        println!("local Crèche component={component} canonical_face_bytes={} encounter_bytes={bytes} output_capacity={}",
            serde_json::to_vec(&snapshot["face"]).unwrap().len(),transport::conduit_creche_local_face_output_capacity());
        acknowledge(&mut current, &snapshot);
    }
}

#[test]
fn malformed_oversized_duplicate_and_disabled_submissions_do_not_mutate_owner() {
    let mut current = current();
    let snapshot = prepare(&mut current, false);
    acknowledge(&mut current, &snapshot);
    let before = current.draft.as_ref().unwrap().friendly_name().to_owned();
    let revision = current.draft.as_ref().unwrap().revision();
    let mut malformed = proposal(&snapshot, "x");
    malformed["arguments"][0]["value"] = json!([255]);
    let mut duplicate = proposal(&snapshot, "x");
    duplicate["arguments"] = json!([duplicate["arguments"][0], duplicate["arguments"][0]]);
    for input in [malformed, duplicate, proposal(&snapshot, &"x".repeat(65))] {
        assert!(current
            .submit_local_face(Submit {
                generation: current.generation,
                interaction: input
            })
            .is_err());
        assert_eq!(current.draft.as_ref().unwrap().friendly_name(), before);
        assert_eq!(current.draft.as_ref().unwrap().revision(), revision);
    }
    current
        .draft
        .as_mut()
        .unwrap()
        .edit_name(revision, "".into())
        .unwrap();
    // A current source revision can change outside this local Mask. Even a
    // well-typed old proposal must still be rejected by the actual owner.
    assert!(current
        .submit_local_face(Submit {
            generation: current.generation,
            interaction: proposal(&snapshot, "stale")
        })
        .is_err());
    assert_eq!(current.draft.as_ref().unwrap().friendly_name(), "");
    let disabled = prepare(&mut current, false);
    acknowledge(&mut current, &disabled);
    let action = disabled["face"]["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|a| a["identity"] == "creche.birth")
        .unwrap();
    let input = json!({"show_id":disabled["view"]["show_id"],"face_id":disabled["view"]["face_id"],
        "face_revision":disabled["view"]["face_revision"],"action_id":action["identity"],"target":action["target"],"arguments":[],"sequence":1});
    assert!(current
        .submit_local_face(Submit {
            generation: current.generation,
            interaction: input
        })
        .is_err());
    assert_eq!(current.draft.as_ref().unwrap().friendly_name(), "");
    assert!(super::super::super::session::current().is_none());
}

#[test]
fn unsupported_valid_text_facts_keep_generic_face_without_control_authority() {
    let mut current = current();
    let snapshot = prepare(&mut current, false);
    let original: conduit_face::Face = serde_json::from_value(snapshot["face"].clone()).unwrap();
    let name_target = original
        .actions
        .iter()
        .find(|action| action.identity == "creche.name")
        .unwrap()
        .target
        .clone();
    let properties = original
        .properties
        .into_iter()
        .filter(|property| {
            property.subject != name_target || property.name != "value-change-action"
        })
        .collect();
    let face = conduit_face::Face::new_with_semantics(
        original.revision,
        original.basis,
        original.subjects,
        original.relationships,
        properties,
        original.text,
        original.actions,
        original.disclosures,
    )
    .unwrap();
    let bindings = serde_json::to_value(control_bindings::ControlBindings::prepare(&face)).unwrap();
    assert!(!bindings["text_entries"]
        .as_array()
        .unwrap()
        .contains(&json!(name_target)));
    assert!(bindings["unsupported"]
        .as_array()
        .unwrap()
        .iter()
        .any(|refusal| refusal["subject"] == name_target && refusal["contract"] == "text-entry"));
    assert!(face
        .actions
        .iter()
        .any(|action| action.identity == "creche.name"));
}

#[test]
fn optional_back_changes_only_realization_of_identical_current_source_face() {
    let mut current = current();
    let generic = prepare(&mut current, false);
    acknowledge(&mut current, &generic);
    let before = serde_json::to_vec(&generic["face"]).unwrap();
    let selected = prepare(&mut current, true);
    assert_eq!(serde_json::to_vec(&selected["face"]).unwrap(), before);
    assert_eq!(selected["face"]["identity"], generic["face"]["identity"]);
    assert_ne!(
        selected["view"]["mask_plan_id"],
        generic["view"]["mask_plan_id"]
    );
    assert_ne!(selected["view"]["show_id"], generic["view"]["show_id"]);
    assert_eq!(
        selected["replaced_show"]["show_id"],
        generic["view"]["show_id"]
    );
    assert_eq!(selected["replaced_show"]["lifecycle"], "Replaced");
    let placement = &selected["planned_mask"]["plan"]["fragments"][0]["placements"][0];
    let pin = serde_json::to_value(crate::owner_face_mask::local_component::descriptor()).unwrap();
    assert_eq!(placement["artifact_id"], pin["artifact_id"]);
    acknowledge(&mut current, &selected);
    assert!(current
        .submit_local_face(Submit {
            generation: current.generation,
            interaction: proposal(&generic, "stale")
        })
        .is_err());
    assert_eq!(current.draft.as_ref().unwrap().revision(), 1);
}

#[test]
fn unpinned_component_prepare_preserves_previous_show_and_source() {
    let mut current = current();
    let generic = prepare(&mut current, false);
    acknowledge(&mut current, &generic);
    let mut admission = crate::owner_face_mask::local_component::expected_admission();
    admission.bundle_sha256 = "0".repeat(64);
    assert!(current
        .prepare_local_face(Prepare {
            generation: current.generation,
            host_id: "host/local-creche".into(),
            boot_id: "boot/local-creche".into(),
            component: true,
            component_admission: Some(admission)
        })
        .is_err());
    assert_eq!(
        current.local_face.as_ref().unwrap().snapshot()["view"]["show_id"],
        generic["view"]["show_id"]
    );
    current
        .submit_local_face(Submit {
            generation: current.generation,
            interaction: proposal(&generic, "still current"),
        })
        .unwrap();
    assert_eq!(
        current.draft.as_ref().unwrap().friendly_name(),
        "still current"
    );
}
