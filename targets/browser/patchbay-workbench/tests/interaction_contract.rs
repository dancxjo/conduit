use conduit_core::ExpandedPlotId;
use conduit_patchbay_workbench::{
    PatchbayAction, PatchbayInteractionRequest, PatchbayInteractionRequestId, PatchbaySubjectRef,
};

fn presentation() -> conduit_presentation::Presentation {
    conduit_presentation::Presentation::new_with_semantics(
        3,
        conduit_presentation::PresentationBasis {
            body_id: None,
            wake_id: None,
            source_document_id: None,
            checked_plot_id: None,
            expanded_plot_id: None,
            plan_id: None,
            active_play_id: None,
            sign_ids: vec![],
        },
        vec![conduit_presentation::PresentationSubject {
            identity: "body/example".into(),
            role: conduit_presentation::PresentationRole::Body,
            name: "Example Body".into(),
        }],
        vec![],
        vec![],
        vec![],
        vec![conduit_presentation::PresentationAction {
            identity: "action/birth/example".into(),
            intent: PatchbayAction::Birth.presentation_intent().into(),
            target: "body/example".into(),
            name: "Birth".into(),
            arguments: vec![],
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
        expanded_plot_id: ExpandedPlotId::from("expanded/example"),
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
            expanded_plot_id,
            subject_identity,
            ..
        } if expanded_plot_id == subject.expanded_plot_id
            && subject_identity == subject.subject_identity
    ));
    assert!(matches!(
        invocation,
        PatchbayInteractionRequest::Invoke { invocation, .. }
            if invocation.action == PatchbayAction::Birth
                && invocation.target_identity == "body/example"
    ));
}
