use conduit_core::ExpandedFormId;
use patchbay_model::{
    PatchbayAction, PatchbayInteractionRequest, PatchbayInteractionRequestId, PatchbaySubjectRef,
};

fn presentation() -> conduit_presentation::Presentation {
    conduit_presentation::Presentation::new_with_semantics(
        3,
        conduit_presentation::PresentationBasis {
            body_id: None,
            wake_id: None,
            source_document_id: None,
            checked_form_id: None,
            expanded_form_id: None,
            plan_id: None,
            active_play_id: None,
            sign_ids: vec![],
        },
        vec![conduit_presentation::PresentationSubject {
            identity: "body/example".into(),
            role: conduit_presentation::PresentationRole::Body,
            label: "Example".into(),
            accessibility_name: "Example Body".into(),
        }],
        vec![],
        vec![],
        vec![],
        vec![conduit_presentation::PresentationAction {
            identity: "action/birth/example".into(),
            intent: PatchbayAction::Birth.presentation_intent().into(),
            target: "body/example".into(),
            label: "Birth".into(),
            disclosure: conduit_presentation::PresentationDisclosureLevel::CurrentAction,
            availability: conduit_presentation::PresentationActionAvailability::Available,
        }],
        vec![],
    )
    .unwrap()
}

#[test]
fn html_can_emit_the_shared_semantic_contract_without_dom_identity() {
    let subject = PatchbaySubjectRef {
        expanded_form_id: ExpandedFormId::from("expanded/example"),
        subject_identity: "gear/source".into(),
    };
    let selection = PatchbayInteractionRequest::select(
        PatchbayInteractionRequestId::new("html/select/1").unwrap(),
        &subject,
    )
    .unwrap();
    let presentation = presentation();
    let invocation = PatchbayInteractionRequest::invoke(
        PatchbayInteractionRequestId::new("html/birth/2").unwrap(),
        &presentation,
        "action/birth/example",
    )
    .unwrap();

    assert!(matches!(
        selection,
        PatchbayInteractionRequest::Select {
            expanded_form_id,
            subject_identity,
            ..
        } if expanded_form_id == subject.expanded_form_id
            && subject_identity == subject.subject_identity
    ));
    assert!(matches!(
        invocation,
        PatchbayInteractionRequest::Invoke { invocation, .. }
            if invocation.action == PatchbayAction::Birth
                && invocation.target_identity == "body/example"
    ));
}
