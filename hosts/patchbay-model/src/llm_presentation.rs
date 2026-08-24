//! Shared semantic Presentation of exact LLM Gear truth.

use conduit_ai::{
    CandidateFormProvenance, CandidateLifecycle, EffectReceipt, LlmSemanticContract,
    LocalModelOffer, ModelDerivedResult, ModelEffectProposal, ModelResultProvenance,
    ProposalDecision, ProposalDecisionOutcome, MAXIMUM_EFFECT_ARGUMENT_BYTES,
    MAXIMUM_PROPOSAL_EVIDENCE, MAXIMUM_PROPOSAL_RATIONALE_BYTES,
};
use conduit_core::PlannedGear;
use conduit_presentation::{
    Presentation, PresentationBasis, PresentationDisclosureLevel, PresentationError,
    PresentationRole,
};

#[path = "llm_presentation_content.rs"]
mod content;
use content::Content;

pub const MAXIMUM_LLM_PRESENTATION_STAGES: usize = 16;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LlmGearActivity {
    Waiting,
    Running,
    Completed,
    Refused,
    Truncated,
    Cancelled,
    ProviderLost,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateFormInspection {
    pub candidate_identity: String,
    pub provenance: CandidateFormProvenance,
    pub lifecycle: CandidateLifecycle,
    pub source_document_identity: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LlmPatchbayTruth<'a> {
    pub gear_identity: String,
    pub contract: &'a LlmSemanticContract,
    pub placement: Option<&'a PlannedGear>,
    pub model_offer: Option<&'a LocalModelOffer>,
    pub activity: LlmGearActivity,
    pub result: Option<&'a ModelDerivedResult>,
    pub candidate_form: Option<&'a CandidateFormInspection>,
    pub proposals: &'a [ModelEffectProposal],
    pub decisions: &'a [ProposalDecision],
    pub effects: &'a [EffectReceipt],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LlmPresentationError {
    TooManyStages,
    InvalidModelOffer,
    ContractMismatch,
    ResultMismatch,
    CandidateMismatch,
    EffectMismatch,
    InvalidPresentation(PresentationError),
}

impl core::fmt::Display for LlmPresentationError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "LLM Patchbay projection refused: {self:?}")
    }
}

impl std::error::Error for LlmPresentationError {}

/// Projects LLM facts into shared Presentation without owning lifecycle, evidence, or authority.
pub fn project_llm_patchbay(
    revision: u64,
    basis: PresentationBasis,
    truth: &LlmPatchbayTruth<'_>,
) -> Result<Presentation, LlmPresentationError> {
    if truth
        .proposals
        .len()
        .saturating_add(truth.decisions.len())
        .saturating_add(truth.effects.len())
        > MAXIMUM_LLM_PRESENTATION_STAGES
    {
        return Err(LlmPresentationError::TooManyStages);
    }
    if truth
        .proposals
        .iter()
        .any(|proposal| basis.plan_id.as_ref() != Some(&proposal.plan_id))
    {
        return Err(LlmPresentationError::ContractMismatch);
    }
    validate_truth(truth)?;
    let form = basis
        .expanded_form_id
        .as_ref()
        .map(|id| format!("form/{}", id.as_str()));
    if basis.body_id.is_some() && form.is_none() {
        return Err(LlmPresentationError::ContractMismatch);
    }

    let body = basis
        .body_id
        .as_ref()
        .map(|id| format!("body/{}", id.as_str()))
        .unwrap_or_else(|| "patchbay/llm-candidate".into());
    let mut content = Content::new();
    content.subject(
        body.clone(),
        if basis.body_id.is_some() {
            PresentationRole::Body
        } else {
            PresentationRole::Document
        },
        "LLM Body",
        "Body containing ordinary typed LLM work",
    );
    if let Some(form) = form {
        content.subject(
            form.clone(),
            PresentationRole::Form,
            "Program Form",
            "Exact expanded Program Form containing the LLM Gear",
        );
        content.contains(&body, &form);
    }
    content.subject(
        truth.gear_identity.clone(),
        PresentationRole::Gear,
        truth.contract.kind_id.as_str(),
        format!("{} semantic Gear", truth.contract.kind_id.as_str()),
    );
    content.contains(&body, &truth.gear_identity);
    content.identity(&truth.gear_identity, "semantic-id", &truth.gear_identity);
    content.identity(
        &truth.gear_identity,
        "kind-id",
        truth.contract.kind_id.as_str(),
    );
    content.text(
        &truth.gear_identity,
        format!("Activity: {:?}", truth.activity),
    );
    content.text_property(
        &truth.gear_identity,
        "activity",
        format!("{:?}", truth.activity),
    );
    content.text_property(
        &truth.gear_identity,
        "evidence-class",
        "semantic Gear state",
    );
    content.disclose(&truth.gear_identity, PresentationDisclosureLevel::Primary);

    for port in truth.contract.inputs.iter().chain(&truth.contract.outputs) {
        let identity = format!("{}/port/{}", truth.gear_identity, port.port_id.as_str());
        content.subject(
            identity.clone(),
            PresentationRole::Port,
            port.port_id.as_str(),
            format!(
                "{} {} carrying {}",
                truth.contract.kind_id.as_str(),
                port.port_id.as_str(),
                port.value_kind.as_str()
            ),
        );
        content.contains(&truth.gear_identity, &identity);
        content.identity(&identity, "semantic-id", port.port_id.as_str());
        content.text_property(
            &identity,
            "direction",
            match port.direction {
                conduit_core::PortDirection::Input => "receiving",
                conduit_core::PortDirection::Output => "outgoing",
            },
        );
        content.identity(&identity, "value-kind", port.value_kind.as_str());
        content.text_property(&identity, "temporal", format!("{:?}", port.temporal));
    }

    append_realization(&mut content, truth);
    append_result(&mut content, truth)?;
    append_candidate(&mut content, truth)?;
    append_request_stages(&mut content, truth)?;

    Presentation::new_with_semantics(
        revision,
        basis,
        content.subjects,
        content.relationships,
        content.properties,
        content.text,
        Vec::new(),
        content.disclosures,
    )
    .map_err(LlmPresentationError::InvalidPresentation)
}

fn validate_truth(truth: &LlmPatchbayTruth<'_>) -> Result<(), LlmPresentationError> {
    if truth.gear_identity.is_empty() {
        return Err(LlmPresentationError::ContractMismatch);
    }
    if let Some(placement) = truth.placement {
        if placement.kind_id != truth.contract.kind_id
            || placement.kind_contract_revision != truth.contract.kind_contract_revision
            || placement.inputs != truth.contract.inputs
            || placement.outputs != truth.contract.outputs
        {
            return Err(LlmPresentationError::ContractMismatch);
        }
    }
    if let Some(offer) = truth.model_offer {
        offer
            .validate()
            .map_err(|_| LlmPresentationError::InvalidModelOffer)?;
    }
    if let Some(result) = truth.result {
        result
            .validate(truth.contract)
            .map_err(|_| LlmPresentationError::ResultMismatch)?;
        if result.provenance != ModelResultProvenance::ModelDerived {
            return Err(LlmPresentationError::ResultMismatch);
        }
    }
    Ok(())
}

fn append_realization(content: &mut Content, truth: &LlmPatchbayTruth<'_>) {
    let Some(placement) = truth.placement else {
        content.text_property(&truth.gear_identity, "realization", "not selected");
        return;
    };
    for (name, value) in [
        ("placement-id", placement.placement_id.as_str()),
        ("host-id", placement.host_id.as_str()),
        ("boot-id", placement.boot_id.as_str()),
        ("capability-id", placement.capability_id.as_str()),
        ("implementation-id", placement.implementation_id.as_str()),
        ("artifact-id", placement.artifact_id.as_str()),
        (
            "execution-profile-id",
            placement.execution_profile_id.as_str(),
        ),
    ] {
        content.identity(&truth.gear_identity, name, value);
    }
    content.count(
        &truth.gear_identity,
        "offer-generation",
        placement.offer_generation.0,
    );
    content.count(
        &truth.gear_identity,
        "maximum-queue-items",
        u64::from(placement.limits.max_queue_items),
    );
    content.count(
        &truth.gear_identity,
        "maximum-queue-bytes",
        u64::from(placement.limits.max_queue_bytes),
    );
    if let Some(offer) = truth.model_offer {
        for (name, value) in [
            ("runtime-name", offer.identity.runtime_name.as_str()),
            ("runtime-version", offer.identity.runtime_version.as_str()),
            ("model-name", offer.identity.model_name.as_str()),
            (
                "model-content-id",
                offer.identity.model_content_identity.as_str(),
            ),
            ("quantization", offer.identity.quantization.as_str()),
        ] {
            content.text_property(&truth.gear_identity, name, value);
        }
        content.count(
            &truth.gear_identity,
            "maximum-input-bytes",
            offer.limits.work.maximum_input_bytes,
        );
        content.count(
            &truth.gear_identity,
            "maximum-context-items",
            offer.limits.work.maximum_context_items,
        );
        content.count(
            &truth.gear_identity,
            "maximum-output-bytes",
            offer.limits.work.maximum_output_bytes,
        );
        content.count(
            &truth.gear_identity,
            "maximum-work-units",
            offer.limits.work.maximum_work_units,
        );
    }
}

fn append_result(
    content: &mut Content,
    truth: &LlmPatchbayTruth<'_>,
) -> Result<(), LlmPresentationError> {
    let Some(result) = truth.result else {
        return Ok(());
    };
    let identity = format!(
        "{}/model-result/{}",
        truth.gear_identity, result.run_identity
    );
    content.subject(
        identity.clone(),
        PresentationRole::Info,
        "MODEL INFO",
        "Model-derived Info, not a system Sign",
    );
    content.describes(&identity, &truth.gear_identity);
    content.text_property(&identity, "evidence-class", "MODEL-DERIVED INFO");
    content.text(
        &identity,
        "MODEL-DERIVED INFO: interpretation/proposal output, not system evidence",
    );
    content.identity(&identity, "request-id", &result.request_identity);
    content.identity(&identity, "run-id", &result.run_identity);
    content.identity(
        &identity,
        "implementation-id",
        &result.implementation_identity,
    );
    content.identity(&identity, "payload-kind", &result.payload_kind);
    content.text_property(
        &identity,
        "disposition",
        format!("{:?}", result.disposition),
    );
    content.count(&identity, "input-bytes", result.accounting.input_bytes);
    content.count(&identity, "output-bytes", result.accounting.output_bytes);
    content.disclose(&identity, PresentationDisclosureLevel::Primary);
    Ok(())
}

fn append_candidate(
    content: &mut Content,
    truth: &LlmPatchbayTruth<'_>,
) -> Result<(), LlmPresentationError> {
    let Some(candidate) = truth.candidate_form else {
        return Ok(());
    };
    if candidate.candidate_identity.is_empty()
        || candidate.source_document_identity.is_empty()
        || candidate.provenance.request_identity.is_empty()
        || candidate.provenance.run_identity.is_empty()
    {
        return Err(LlmPresentationError::CandidateMismatch);
    }
    content.subject(
        candidate.candidate_identity.clone(),
        PresentationRole::Candidate,
        "Candidate Form",
        "Model-produced candidate Form open in the ordinary editor and not running",
    );
    content.describes(&candidate.candidate_identity, &truth.gear_identity);
    content.text_property(
        &candidate.candidate_identity,
        "candidate-state",
        "OPEN AND EDITABLE",
    );
    content.text_property(
        &candidate.candidate_identity,
        "lifecycle",
        format!("{:?}", candidate.lifecycle),
    );
    content.identity(
        &candidate.candidate_identity,
        "source-document-id",
        &candidate.source_document_identity,
    );
    content.identity(
        &candidate.candidate_identity,
        "request-id",
        &candidate.provenance.request_identity,
    );
    content.identity(
        &candidate.candidate_identity,
        "run-id",
        &candidate.provenance.run_identity,
    );
    content.text_property(&candidate.candidate_identity, "auto-run", "false");
    content.disclose(
        &candidate.candidate_identity,
        PresentationDisclosureLevel::CurrentAction,
    );
    Ok(())
}

fn append_request_stages(
    content: &mut Content,
    truth: &LlmPatchbayTruth<'_>,
) -> Result<(), LlmPresentationError> {
    for proposal in truth.proposals {
        if proposal.proposal_id.is_empty()
            || proposal.canonical_arguments.is_empty()
            || proposal.canonical_arguments.len() > MAXIMUM_EFFECT_ARGUMENT_BYTES
            || proposal.rationale.len() > MAXIMUM_PROPOSAL_RATIONALE_BYTES
            || proposal.evidence.len() > MAXIMUM_PROPOSAL_EVIDENCE
        {
            return Err(LlmPresentationError::EffectMismatch);
        }
        content.subject(
            proposal.proposal_id.clone(),
            PresentationRole::Status,
            "MODEL PROPOSAL",
            "Model-derived request awaiting an authority decision",
        );
        content.describes(&proposal.proposal_id, &truth.gear_identity);
        content.text_property(&proposal.proposal_id, "stage", "AWAITING AUTHORITY");
        content.text_property(
            &proposal.proposal_id,
            "evidence-class",
            "MODEL-DERIVED REQUEST",
        );
        content.identity(&proposal.proposal_id, "plan-id", proposal.plan_id.as_str());
        content.identity(
            &proposal.proposal_id,
            "operation-kind",
            proposal.operation_kind.as_str(),
        );
        content.count(
            &proposal.proposal_id,
            "argument-bytes",
            proposal.canonical_arguments.len() as u64,
        );
        content.text(&proposal.proposal_id, &proposal.rationale);
        content.disclose(&proposal.proposal_id, PresentationDisclosureLevel::Primary);
    }
    for decision in truth.decisions {
        if decision.decision_id.is_empty()
            || !truth
                .proposals
                .iter()
                .any(|proposal| proposal.proposal_id == decision.proposal_id)
        {
            return Err(LlmPresentationError::EffectMismatch);
        }
        content.subject(
            decision.decision_id.clone(),
            PresentationRole::Status,
            "MODEL REQUEST DECISION",
            "Authority decision for a model-derived proposal",
        );
        content.describes(&decision.decision_id, &truth.gear_identity);
        content.identity(&decision.decision_id, "proposal-id", &decision.proposal_id);
        content.text_property(&decision.decision_id, "stage", "PROPOSAL DECIDED");
        match &decision.outcome {
            ProposalDecisionOutcome::Authorized { request_id } => {
                content.identity(&decision.decision_id, "request-id", request_id);
                content.text_property(&decision.decision_id, "authority-state", "ADMITTED");
            }
            ProposalDecisionOutcome::Refused(refusal) => {
                content.text_property(&decision.decision_id, "authority-state", "REFUSED");
                content.text_property(&decision.decision_id, "refusal", format!("{refusal:?}"));
            }
        }
        content.disclose(
            &decision.decision_id,
            PresentationDisclosureLevel::CurrentAction,
        );
    }
    for effect in truth.effects {
        if effect.effect_id.is_empty()
            || !truth.decisions.iter().any(|decision| {
                matches!(&decision.outcome, ProposalDecisionOutcome::Authorized { request_id } if request_id == &effect.request_id)
            })
        {
            return Err(LlmPresentationError::EffectMismatch);
        }
        let sign_subject = format!("{}/system-sign", effect.effect_id);
        content.subject(
            sign_subject.clone(),
            PresentationRole::Sign,
            "SYSTEM SIGN",
            "System evidence produced after an admitted effect request",
        );
        content.describes(&sign_subject, &truth.gear_identity);
        content.text_property(&sign_subject, "evidence-class", "SYSTEM SIGN EVIDENCE");
        content.identity(&sign_subject, "effect-id", &effect.effect_id);
        content.identity(&sign_subject, "request-id", &effect.request_id);
        for (index, sign) in effect.resulting_signs.iter().enumerate() {
            content.identity(&sign_subject, &format!("sign-{index}"), sign.as_str());
        }
        content.disclose(&sign_subject, PresentationDisclosureLevel::Primary);
    }
    Ok(())
}

#[cfg(test)]
#[path = "llm_presentation_tests.rs"]
mod tests;
