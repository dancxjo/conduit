use super::*;

#[test]
fn partial_native_receipts_are_not_checkpoints() {
    let serial = concat!(
        "CONDUIT_NATIVE_ACTION {\"requested_intent\":\"conduit.intent/wake@1\",\"disposition\":{\"Refused\":{\"reason\":\"unavailable\"}}}\n",
        "CONDUIT_NATIVE_ACTION {\"requested_intent\":\"conduit.intent/wake@1\""
    );
    assert_eq!(refused_wake_count(serial).unwrap(), 1);
}

#[test]
fn malformed_complete_native_receipts_fail_closed() {
    assert!(records("CONDUIT_NATIVE_FACE invalid\n", FACE_PREFIX).is_err());
    let serial = "CONDUIT_NATIVE_FACE {}\n".repeat(MAX_RECORDS + 1);
    assert!(records(&serial, FACE_PREFIX).is_err());
}

#[test]
fn refused_owner_receipt_cannot_hide_mutation_or_an_accepted_effect() {
    let owner = json!({"body_id":"body/a", "born_sign_id":"born/a", "host_id":"host/a", "boot_id":"boot/a", "revision":7});
    let record = json!({"before":owner, "after":owner,
            "accepted_effects":0, "disposition":{"Refused":{"reason":"UnavailableAction"}}});
    validate_owner_effect(&record).unwrap();
    let mut changed = record.clone();
    changed["after"]["revision"] = json!(8);
    assert!(validate_owner_effect(&changed).is_err());
    let mut forged = record;
    forged["accepted_effects"] = json!(1);
    assert!(validate_owner_effect(&forged).is_err());
}

#[test]
fn accepted_owner_receipt_requires_original_birth_and_terminal_operation() {
    let before = json!({"body_id":"body/a", "born_sign_id":"born/a", "host_id":"host/a", "boot_id":"boot/a", "revision":7});
    let mut after = before.clone();
    after["revision"] = json!(8);
    after["last_request_id"] = json!("request/8");
    let record = json!({"before":before,"after":after,"accepted_effects":1,
            "disposition":{"Accepted":{"operation_request_id":"request/8"}}});
    validate_owner_effect(&record).unwrap();
    let mut reborn = record.clone();
    reborn["after"]["born_sign_id"] = json!("born/another");
    assert!(validate_owner_effect(&reborn).is_err());
    let mut uncommitted = record;
    uncommitted["disposition"]["Accepted"]["operation_request_id"] = Value::Null;
    assert!(validate_owner_effect(&uncommitted).is_err());
}

#[test]
fn recorded_arguments_obey_the_actual_source_value_contract() {
    let action = FaceAction {
        identity: "action/name".into(),
        intent: "intent/name".into(),
        target: "body/a".into(),
        name: "Name".into(),
        arguments: vec![
            conduit_face::FaceActionArgument::text("name".into(), "Name".into(), 1, 4).unwrap(),
        ],
        disclosure: conduit_face::FaceDisclosureLevel::Primary,
        availability: FaceActionAvailability::Available,
    };
    // Receipt-shape fixture only; this test cannot admit an owner interaction.
    let mut interaction: FaceInteraction = serde_json::from_value(json!({
        "identity":"interaction/fixture", "face_id":"face/fixture", "face_revision":1,
        "show_id":"show/fixture", "action_id":"action/name", "target":"body/a", "sequence":1,
        "arguments":[{"name":"name", "value_kind":conduit_face::UTF8_TEXT_VALUE_KIND,"value":[65]}],
    }))
    .unwrap();
    validate_arguments(&action, &interaction).unwrap();
    interaction.arguments[0].value = vec![65; 5];
    assert!(validate_arguments(&action, &interaction).is_err());
    interaction.arguments[0].value = vec![65];
    interaction.arguments[0].value_kind = "kind/wrong".into();
    assert!(validate_arguments(&action, &interaction).is_err());
    interaction.arguments[0].value_kind = conduit_face::UTF8_TEXT_VALUE_KIND.into();
    interaction.arguments.push(interaction.arguments[0].clone());
    assert!(validate_arguments(&action, &interaction).is_err());
}

#[test]
fn a_future_show_cannot_retroactively_authorize_a_recorded_action() {
    let serial = concat!(
        "CONDUIT_NATIVE_ACTION {\"show_id\":\"show/later\"}\n",
        "CONDUIT_NATIVE_FACE {\"show\":{\"show_id\":\"show/later\"}}\n",
    );
    assert!(validate_receipt_order(serial).is_err());
}

// Canonical Face/receipt fixtures test evidence rejection, not installed effects.
fn admission_fixture() -> (Face, Value) {
    use conduit_face::{FaceBasis, FaceProperty, FacePropertyValue, FaceRole, FaceSubject};
    let make = |revision, input_count| {
        Face::new_with_semantics(
            revision,
            FaceBasis {
                body_id: Some(serde_json::from_value(json!("body/a")).unwrap()),
                wake_id: None,
                source_document_id: None,
                checked_plot_id: None,
                expanded_plot_id: None,
                plan_id: None,
                active_play_id: None,
                sign_ids: vec![],
            },
            vec![
                FaceSubject {
                    identity: "host/a".into(),
                    role: FaceRole::Host,
                    name: "Host".into(),
                },
                FaceSubject {
                    identity: "body/body/a".into(),
                    role: FaceRole::Body,
                    name: "Body".into(),
                },
            ],
            vec![],
            vec![
                FaceProperty {
                    subject: "host/a".into(),
                    name: "host-id".into(),
                    value: FacePropertyValue::Identity("host/a".into()),
                },
                FaceProperty {
                    subject: "host/a".into(),
                    name: "boot-id".into(),
                    value: FacePropertyValue::Identity("boot/a".into()),
                },
                FaceProperty {
                    subject: "host/a".into(),
                    name: "offer-generation".into(),
                    value: FacePropertyValue::Count(1),
                },
                FaceProperty {
                    subject: "host/a".into(),
                    name: "input-events".into(),
                    value: FacePropertyValue::Count(input_count),
                },
            ],
            vec![],
            vec![FaceAction {
                identity: "action/stop".into(),
                intent: "intent/stop".into(),
                target: "body/body/a".into(),
                name: "Stop".into(),
                arguments: vec![],
                disclosure: conduit_face::FaceDisclosureLevel::CurrentAction,
                availability: FaceActionAvailability::Available,
            }],
            vec![],
        )
        .unwrap()
    };
    let source = make(3, 0);
    let admission = make(4, 1);
    let owner = json!({"body_id":"body/a","born_sign_id":"born/a","host_id":"host/a","boot_id":"boot/a",
        "offer_generation":1,"input_count":1,"revision":8,"wake_id":null,"source_document_id":null,
        "checked_plot_id":null,"expanded_plot_id":null,"plan_id":null,"active_play_id":null});
    let interaction = json!({"identity":"interaction/fixture","show_id":"show/source","face_id":source.identity,
        "face_revision":source.revision,"action_id":"action/stop","target":"body/body/a","arguments":[],"sequence":1});
    (
        source.clone(),
        json!({"schema":"conduit.native-action/v1","face_id":source.identity,"face_revision":source.revision,
        "show_id":"show/source","requested_intent":"intent/stop","interaction":interaction,"admission_face":admission,
        "before":owner,"after":owner,"accepted_effects":0,"disposition":{"Refused":{"reason":"StaleFace"}}}),
    )
}

#[test]
fn stale_admission_requires_valid_newer_face_and_exact_current_owner_facts() {
    let (source, action) = admission_fixture();
    validate_admission_face(&source, &action).unwrap();
    let mut tampered = action.clone();
    tampered["admission_face"]["subjects"][0]["name"] = json!("forged unbound name");
    assert!(validate_admission_face(&source, &tampered).is_err());
    let mut unchanged = action.clone();
    unchanged["admission_face"] = json!(source);
    assert!(validate_admission_face(&source, &unchanged).is_err());
    let mut wrong_owner = action;
    wrong_owner["before"]["input_count"] = json!(99);
    assert!(validate_admission_face(&source, &wrong_owner).is_err());
}

#[test]
fn stale_refusal_rejects_nonzero_effect_and_current_face_rebased_interaction() {
    let (source, mut action) = admission_fixture();
    validate_owner_effect(&action).unwrap();
    validate_source_interaction(&source, &action).unwrap();
    action["accepted_effects"] = json!(1);
    assert!(validate_owner_effect(&action).is_err());
    action["accepted_effects"] = json!(0);
    action["interaction"]["face_id"] = action["admission_face"]["identity"].clone();
    action["interaction"]["face_revision"] = action["admission_face"]["revision"].clone();
    assert!(validate_source_interaction(&source, &action).is_err());
}
