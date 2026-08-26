//! Shared Presentation views of graph-defined model perception and power.

use conduit_ai::{
    EmbodiedModelReceipt, EmbodiedModelView, EmbodimentStage, ProposalDecisionOutcome,
    ProposalRefusal,
};
use conduit_body::Body;
use conduit_core::{
    kind_id, ActivePlayId, CheckedFormId, ExpandedFormId, PlanId, SignId, SourceDocumentId,
};
use conduit_presentation::{
    Presentation, PresentationBasis, PresentationDisclosureLevel, PresentationError,
    PresentationRole,
};

use crate::llm_presentation::content::Content;

pub fn project_llm_embodiment(
    first_revision: u64,
    body: &Body,
    receipt: &EmbodiedModelReceipt,
) -> Result<Vec<Presentation>, LlmEmbodimentPresentationError> {
    receipt
        .validate()
        .map_err(|_| LlmEmbodimentPresentationError::InvalidReceipt)?;
    if body.body_id.as_str() != receipt.body_id {
        return Err(LlmEmbodimentPresentationError::InvalidReceipt);
    }
    receipt
        .views
        .iter()
        .enumerate()
        .map(|(index, view)| project_view(first_revision + index as u64, body, receipt, view))
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LlmEmbodimentPresentationError {
    InvalidReceipt,
    InvalidPresentation(PresentationError),
}

pub fn llm_embodiment_documentary_presentations(
) -> Result<Vec<Presentation>, LlmEmbodimentPresentationError> {
    let body = Body::born(
        SourceDocumentId::from("source/llm-embodiment-documentary"),
        CheckedFormId::from("checked/llm-embodiment-documentary"),
        1,
        SignId::from("sign/llm-embodiment-born"),
    )
    .map_err(|_| LlmEmbodimentPresentationError::InvalidReceipt)?;
    let views = [
        EmbodimentStage::PerceptionOnly,
        EmbodimentStage::Expressive,
        EmbodimentStage::AuthorizedEffect,
    ]
    .into_iter()
    .enumerate()
    .map(|(index, stage)| documentary_view(stage, index))
    .collect();
    let receipt = EmbodiedModelReceipt {
        schema: "conduit.llm/embodied-body-receipt@1",
        proof_class: "deterministic-documentary-fixture",
        body_id: body.body_id.as_str().into(),
        perception_value_kind: kind_id("perception/scene-summary@1"),
        state_value_kind: kind_id("robotics/battery-state@1"),
        expressive_value_kind: kind_id("value/text@1"),
        protected_effect_kind: kind_id("effect/indicator-set@1"),
        views,
        ambient_host_access: false,
    };
    project_llm_embodiment(70, &body, &receipt)
}

fn documentary_view(stage: EmbodimentStage, index: usize) -> EmbodiedModelView {
    let expressive = index > 0;
    let authorized = index > 1;
    EmbodiedModelView {
        stage,
        checked_form_id: CheckedFormId::from(format!("checked/llm-embodiment/{index}")),
        expanded_form_id: ExpandedFormId::from(format!("expanded/llm-embodiment/{index}")),
        plan_id: PlanId::from(format!("plan/llm-embodiment/{index}")),
        active_play_id: ActivePlayId::from(format!("play/llm-embodiment/{index}")),
        model_gear_identity: "gear/local-model".into(),
        model_implementation_identity: "ollama/gpt-oss:20b/exact-digest".into(),
        wired_inputs: vec![
            kind_id("perception/scene-summary@1"),
            kind_id("robotics/battery-state@1"),
        ],
        wired_outputs: if authorized {
            vec![kind_id("llm/proposal-result@1"), kind_id("value/text@1")]
        } else if expressive {
            vec![kind_id("value/text@1")]
        } else {
            vec![]
        },
        expressive_output_wired: expressive,
        protected_effect_wired: authorized,
        authority_id: authorized.then(|| "grant/indicator-only".into()),
        proposal_id: format!("proposal/llm-embodiment/{index}"),
        decision: if authorized {
            ProposalDecisionOutcome::Authorized {
                request_id: "request/indicator".into(),
            }
        } else {
            ProposalDecisionOutcome::Refused(ProposalRefusal::UnwiredOperation)
        },
        resulting_signs: authorized
            .then(|| SignId::from("sign/indicator-effect"))
            .into_iter()
            .collect(),
    }
}

fn project_view(
    revision: u64,
    body: &Body,
    receipt: &EmbodiedModelReceipt,
    view: &EmbodiedModelView,
) -> Result<Presentation, LlmEmbodimentPresentationError> {
    let body_subject = format!("body/{}", receipt.body_id);
    let form = format!("form/{}", view.expanded_form_id.as_str());
    let gear = format!("{form}/{}", view.model_gear_identity);
    let proposal = format!("{form}/{}", view.proposal_id);
    let decision = format!("{proposal}/decision");
    let mut content = Content::new();
    content.subject(
        body_subject.clone(),
        PresentationRole::Body,
        "Embodied model Body",
        "Body whose exact Form wiring defines model perception and power",
    );
    content.subject(
        form.clone(),
        PresentationRole::Form,
        format!("{:?}", view.stage),
        format!("Exact {:?} model Form", view.stage),
    );
    content.contains(&body_subject, &form);
    content.subject(
        gear.clone(),
        PresentationRole::Gear,
        "Local model Gear",
        "Same realized local model Gear",
    );
    content.contains(&form, &gear);
    content.identity(
        &gear,
        "implementation-id",
        &view.model_implementation_identity,
    );
    content.identity(&gear, "plan-id", view.plan_id.as_str());
    append_ports(&mut content, &gear, view);
    content.subject(
        proposal.clone(),
        PresentationRole::Status,
        "MODEL PROPOSAL",
        "Model-derived proposal awaiting an ordinary authority decision",
    );
    content.describes(&proposal, &gear);
    content.subject(
        decision.clone(),
        PresentationRole::Status,
        "MODEL REQUEST DECISION",
        "Ordinary proposal-gate decision",
    );
    content.describes(&decision, &proposal);
    match &view.decision {
        ProposalDecisionOutcome::Authorized { request_id } => {
            content.text_property(&decision, "authority-state", "ADMITTED");
            content.identity(&decision, "request-id", request_id);
        }
        ProposalDecisionOutcome::Refused(refusal) => {
            content.text_property(&decision, "authority-state", "REFUSED");
            content.text_property(&decision, "refusal", format!("{refusal:?}"));
        }
    }
    for sign in &view.resulting_signs {
        let identity = format!("{form}/sign/{}", sign.as_str());
        content.subject(
            identity.clone(),
            PresentationRole::Sign,
            "SYSTEM SIGN",
            "System effect evidence after admitted execution",
        );
        content.describes(&identity, &decision);
        content.identity(&identity, "sign-id", sign.as_str());
        content.text_property(&identity, "evidence-class", "SYSTEM SIGN EVIDENCE");
    }
    content.disclose(&gear, PresentationDisclosureLevel::Primary);
    content.disclose(&decision, PresentationDisclosureLevel::CurrentAction);
    Presentation::new_with_semantics(
        revision,
        PresentationBasis {
            seed_id: Some(body.seed_id.clone()),
            body_id: Some(body.body_id.clone()),
            wake_id: None,
            source_document_id: Some(body.source_document_id.clone()),
            checked_form_id: Some(view.checked_form_id.clone()),
            expanded_form_id: Some(view.expanded_form_id.clone()),
            plan_id: Some(view.plan_id.clone()),
            active_play_id: Some(view.active_play_id.clone()),
            sign_ids: view.resulting_signs.clone(),
        },
        content.subjects,
        content.relationships,
        content.properties,
        content.text,
        vec![],
        content.disclosures,
    )
    .map_err(LlmEmbodimentPresentationError::InvalidPresentation)
}

fn append_ports(content: &mut Content, gear: &str, view: &EmbodiedModelView) {
    for (direction, kinds) in [
        ("input", &view.wired_inputs),
        ("output", &view.wired_outputs),
    ] {
        for (index, kind) in kinds.iter().enumerate() {
            let port = format!("{gear}/port/{direction}/{index}");
            let cord = format!("{port}/cord");
            content.subject(
                port.clone(),
                PresentationRole::Port,
                format!("{direction}: {}", kind.as_str()),
                format!("Wired {direction} Port carrying {}", kind.as_str()),
            );
            content.contains(gear, &port);
            content.identity(&port, "value-kind", kind.as_str());
            content.subject(
                cord.clone(),
                PresentationRole::Cord,
                "Planned Cord",
                format!("Exact planned Cord carrying {}", kind.as_str()),
            );
            content.connects(&port, &cord);
        }
    }
}
