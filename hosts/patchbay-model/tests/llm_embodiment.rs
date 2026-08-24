use conduit_ai::{
    EmbodiedModelReceipt, EmbodiedModelView, EmbodimentStage, ProposalDecisionOutcome,
    ProposalRefusal,
};
use conduit_body::Body;
use conduit_core::{
    kind_id, ActivePlayId, CheckedFormId, ExpandedFormId, PlanId, SignId, SourceDocumentId,
};
use conduit_presentation::PresentationRole;
use patchbay_model::{project_llm_embodiment, LlmEmbodimentPresentationError};

fn receipt(body: &Body) -> EmbodiedModelReceipt {
    let stages = [
        EmbodimentStage::PerceptionOnly,
        EmbodimentStage::Expressive,
        EmbodimentStage::AuthorizedEffect,
    ];
    let views = stages
        .into_iter()
        .enumerate()
        .map(|(index, stage)| {
            let expressive = index > 0;
            let authorized = index > 1;
            EmbodiedModelView {
                stage,
                checked_form_id: CheckedFormId::from(format!("checked/embodied/{index}")),
                expanded_form_id: ExpandedFormId::from(format!("expanded/embodied/{index}")),
                plan_id: PlanId::from(format!("plan/embodied/{index}")),
                active_play_id: ActivePlayId::from(format!("play/embodied/{index}")),
                model_gear_identity: "gear/model".into(),
                model_implementation_identity: "ollama/gpt-oss:20b/exact-digest".into(),
                wired_inputs: vec![
                    kind_id("perception/scene-summary@1"),
                    kind_id("robotics/battery-state@1"),
                ],
                wired_outputs: expressive
                    .then(|| kind_id("value/text@1"))
                    .into_iter()
                    .collect(),
                expressive_output_wired: expressive,
                protected_effect_wired: authorized,
                authority_id: authorized.then(|| "grant/indicator-only".into()),
                proposal_id: format!("proposal/embodied/{index}"),
                decision: if authorized {
                    ProposalDecisionOutcome::Authorized {
                        request_id: "request/indicator".into(),
                    }
                } else {
                    ProposalDecisionOutcome::Refused(ProposalRefusal::UnwiredOperation)
                },
                resulting_signs: authorized
                    .then(|| SignId::from("sign/indicator"))
                    .into_iter()
                    .collect(),
            }
        })
        .collect();
    EmbodiedModelReceipt {
        schema: "conduit.llm/embodied-body-receipt@1",
        proof_class: "deterministic-production-kernel",
        body_id: body.body_id.as_str().into(),
        perception_value_kind: kind_id("perception/scene-summary@1"),
        state_value_kind: kind_id("robotics/battery-state@1"),
        expressive_value_kind: kind_id("value/text@1"),
        protected_effect_kind: kind_id("effect/indicator-set@1"),
        views,
        ambient_host_access: false,
    }
}

#[test]
fn patchbay_distinguishes_three_forms_ports_decisions_and_effect_sign() {
    let body = Body::born(
        SourceDocumentId::from("source/embodied"),
        CheckedFormId::from("checked/body"),
        1,
        SignId::from("sign/born"),
    )
    .unwrap();
    let presentations = project_llm_embodiment(40, &body, &receipt(&body)).unwrap();

    assert_eq!(presentations.len(), 3);
    assert!(presentations.iter().all(|item| item
        .subjects
        .iter()
        .any(|subject| subject.role == PresentationRole::Form)));
    assert!(presentations[0]
        .subjects
        .iter()
        .all(|subject| subject.role != PresentationRole::Sign));
    assert!(presentations[2]
        .subjects
        .iter()
        .any(|subject| subject.role == PresentationRole::Sign));
    assert!(presentations[2]
        .subjects
        .iter()
        .any(|subject| subject.role == PresentationRole::Cord));
}

#[test]
fn presentation_refuses_a_receipt_for_another_body() {
    let body = Body::born(
        SourceDocumentId::from("source/embodied"),
        CheckedFormId::from("checked/body"),
        1,
        SignId::from("sign/born"),
    )
    .unwrap();
    let other = Body::born(
        SourceDocumentId::from("source/other"),
        CheckedFormId::from("checked/other"),
        2,
        SignId::from("sign/other-born"),
    )
    .unwrap();
    assert_eq!(
        project_llm_embodiment(1, &other, &receipt(&body)),
        Err(LlmEmbodimentPresentationError::InvalidReceipt)
    );
}
