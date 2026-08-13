use conduit_core::ExpandedFormId;
use patchbay_model::{
    PatchbayAction, PatchbayInteractionRequest, PatchbayInteractionRequestId, PatchbaySubjectRef,
};

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
    let invocation = PatchbayInteractionRequest::invoke(
        PatchbayInteractionRequestId::new("html/be-born/2").unwrap(),
        PatchbayAction::BeBorn,
        "body/example",
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
            if invocation.action == PatchbayAction::BeBorn
                && invocation.target_identity == "body/example"
    ));
}
