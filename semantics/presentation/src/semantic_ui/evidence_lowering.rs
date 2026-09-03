use super::{
    ArtifactPresentation, EvidenceDisposition, EvidencePresentation, SemanticPresentationRefusal,
};
use crate::{
    ApplicationComponent, ApplicationNodeState, ApplicationViewNode, ApplicationViewRefusal,
};
use alloc::{format, string::String, vec::Vec};

type LoweredNode = (
    ApplicationComponent,
    String,
    String,
    u32,
    Option<u8>,
    ApplicationNodeState,
);

pub(super) fn evidence_component(disposition: EvidenceDisposition) -> ApplicationComponent {
    match disposition {
        EvidenceDisposition::Missing => ApplicationComponent::MissingEvidence,
        EvidenceDisposition::Stale => ApplicationComponent::StaleEvidence,
        EvidenceDisposition::Refused => ApplicationComponent::RefusedEvidence,
        EvidenceDisposition::Failed => ApplicationComponent::FailedEvidence,
        EvidenceDisposition::Succeeded => ApplicationComponent::SuccessfulEvidence,
    }
}

pub(super) fn validate_evidence(
    evidence: &EvidencePresentation,
) -> Result<(), SemanticPresentationRefusal> {
    if evidence.title.is_empty() || evidence.identity.is_empty() || evidence.provenance.is_empty() {
        return Err(SemanticPresentationRefusal::InvalidEvidence);
    }
    Ok(())
}

pub(super) fn validate_artifact(
    artifact: &ArtifactPresentation,
) -> Result<(), SemanticPresentationRefusal> {
    if artifact.title.is_empty()
        || artifact.kind.is_empty()
        || artifact.detail.is_empty()
        || artifact.identity.is_empty()
        || artifact.provenance.is_empty()
    {
        return Err(SemanticPresentationRefusal::InvalidArtifact);
    }
    Ok(())
}

pub(super) fn definition_node(
    term: &str,
    value: &str,
) -> Result<LoweredNode, SemanticPresentationRefusal> {
    if term.is_empty() {
        return Err(SemanticPresentationRefusal::InvalidDefinition);
    }
    Ok((
        ApplicationComponent::Definition,
        term.into(),
        value.into(),
        value_capacity(value),
        None,
        ApplicationNodeState::Ready,
    ))
}

pub(super) fn code_node(
    language: &str,
    code: &str,
) -> Result<LoweredNode, SemanticPresentationRefusal> {
    if language.is_empty() {
        return Err(SemanticPresentationRefusal::InvalidCodeBlock);
    }
    Ok((
        ApplicationComponent::CodeBlock,
        language.into(),
        code.into(),
        value_capacity(code),
        None,
        ApplicationNodeState::Ready,
    ))
}

pub(super) fn push_definition(
    parent: u8,
    nodes: &mut Vec<ApplicationViewNode>,
    term: &str,
    value: &str,
) -> Result<(), SemanticPresentationRefusal> {
    let index = u8::try_from(nodes.len()).map_err(|_| {
        SemanticPresentationRefusal::ApplicationView(ApplicationViewRefusal::TooManyNodes)
    })?;
    nodes.push(ApplicationViewNode {
        parent: Some(parent),
        component: ApplicationComponent::Definition,
        key: format!("n{parent}-d{index}"),
        text: term.into(),
        value: value.into(),
        value_capacity: value_capacity(value),
        action: None,
        state: ApplicationNodeState::Ready,
    });
    Ok(())
}

pub(super) fn push_evidence_state(
    parent: u8,
    nodes: &mut Vec<ApplicationViewNode>,
    disposition: EvidenceDisposition,
) -> Result<(), SemanticPresentationRefusal> {
    let index = u8::try_from(nodes.len()).map_err(|_| {
        SemanticPresentationRefusal::ApplicationView(ApplicationViewRefusal::TooManyNodes)
    })?;
    nodes.push(ApplicationViewNode {
        parent: Some(parent),
        component: evidence_component(disposition),
        key: format!("n{parent}-s{index}"),
        text: "Artifact evidence".into(),
        value: String::new(),
        value_capacity: 0,
        action: None,
        state: ApplicationNodeState::Ready,
    });
    Ok(())
}

fn value_capacity(value: &str) -> u32 {
    u32::try_from(value.len()).unwrap_or(u32::MAX).max(1)
}
