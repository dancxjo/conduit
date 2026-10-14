use conduit_core::ExpandedPlotId;
use conduit_patchbay_workbench::{
    PatchbayAction, PatchbayInteractionRequest, PatchbayInteractionRequestId, PatchbaySubjectRef,
};

fn face() -> conduit_face::Face {
    conduit_face::Face::new_with_semantics(
        3,
        conduit_face::FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id: None,
            checked_plot_id: None,
            expanded_plot_id: None,
            plan_id: None,
            active_play_id: None,
            sign_ids: vec![],
        },
        vec![conduit_face::FaceSubject {
            identity: "body/example".into(),
            role: conduit_face::FaceRole::Body,
            name: "Example Body".into(),
        }],
        vec![],
        vec![],
        vec![],
        vec![conduit_face::FaceAction {
            identity: "action/birth/example".into(),
            intent: PatchbayAction::Birth.face_intent().into(),
            target: "body/example".into(),
            name: "Birth".into(),
            arguments: vec![],
            disclosure: conduit_face::FaceDisclosureLevel::CurrentAction,
            availability: conduit_face::FaceActionAvailability::Available,
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
    let face = face();
    let invocation = PatchbayInteractionRequest::invoke(
        PatchbayInteractionRequestId::new("html/birth/2").unwrap(),
        &face,
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
