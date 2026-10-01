use crate::*;
use sha2::{Digest, Sha256};

pub(crate) fn hash_correlation(state: &mut Sha256, value: &GeneratedSemanticCorrelation) {
    fn bytes(state: &mut Sha256, value: &str) {
        state.update((value.len() as u64).to_be_bytes());
        state.update(value.as_bytes());
    }
    fn relationship(state: &mut Sha256, value: &PresentationRelationshipKind) {
        match value {
            PresentationRelationshipKind::Contains => state.update([0]),
            PresentationRelationshipKind::Connects => state.update([1]),
            PresentationRelationshipKind::Describes => state.update([2]),
            PresentationRelationshipKind::Realizes => state.update([3]),
            PresentationRelationshipKind::Observes => state.update([4]),
            PresentationRelationshipKind::Semantic(kind) => {
                state.update([5]);
                bytes(state, kind.as_str());
            }
        }
    }
    match value {
        GeneratedSemanticCorrelation::Subject { index, identity } => {
            state.update([0]);
            state.update(index.to_be_bytes());
            bytes(state, identity);
        }
        GeneratedSemanticCorrelation::Relationship {
            index,
            source,
            target,
            kind,
        } => {
            state.update([1]);
            state.update(index.to_be_bytes());
            bytes(state, source);
            bytes(state, target);
            relationship(state, kind);
        }
        GeneratedSemanticCorrelation::Text { index, subject } => {
            state.update([2]);
            state.update(index.to_be_bytes());
            bytes(state, subject);
        }
        GeneratedSemanticCorrelation::Property {
            index,
            subject,
            name,
        } => {
            state.update([3]);
            state.update(index.to_be_bytes());
            bytes(state, subject);
            bytes(state, name);
        }
        GeneratedSemanticCorrelation::TypedContent {
            index,
            subject,
            name,
            content_profile,
        } => {
            state.update([4]);
            state.update(index.to_be_bytes());
            bytes(state, subject);
            bytes(state, name);
            bytes(state, content_profile);
        }
        GeneratedSemanticCorrelation::Composition { index, identity } => {
            state.update([5]);
            state.update(index.to_be_bytes());
            bytes(state, identity);
        }
        GeneratedSemanticCorrelation::Action {
            index,
            identity,
            intent,
            target,
        } => {
            state.update([6]);
            state.update(index.to_be_bytes());
            bytes(state, identity);
            bytes(state, intent);
            bytes(state, target);
        }
        GeneratedSemanticCorrelation::ActionArgument {
            action_index,
            argument_index,
            name,
            value_kind,
        } => {
            state.update([7]);
            state.update(action_index.to_be_bytes());
            state.update(argument_index.to_be_bytes());
            bytes(state, name);
            bytes(state, value_kind);
        }
        GeneratedSemanticCorrelation::Disclosure {
            index,
            subject,
            level,
        } => {
            state.update([8]);
            state.update(index.to_be_bytes());
            bytes(state, subject);
            state.update([match level {
                PresentationDisclosureLevel::Primary => 0,
                PresentationDisclosureLevel::CurrentAction => 1,
                PresentationDisclosureLevel::Context => 2,
                PresentationDisclosureLevel::SelectedDetail => 3,
                PresentationDisclosureLevel::ExactProvenance => 4,
            }]);
        }
        GeneratedSemanticCorrelation::TemporalReference { index, identity } => {
            state.update([9]);
            state.update(index.to_be_bytes());
            bytes(state, identity);
        }
        GeneratedSemanticCorrelation::TemporalFact {
            index,
            subject,
            reference,
            role,
        } => {
            state.update([10]);
            state.update(index.to_be_bytes());
            bytes(state, subject);
            bytes(state, reference);
            state.update([match role {
                PresentationTemporalRole::Event => 0,
                PresentationTemporalRole::Observation => 1,
                PresentationTemporalRole::Ingestion => 2,
            }]);
        }
        GeneratedSemanticCorrelation::Context {
            index,
            source,
            target,
            relationship: kind,
        } => {
            state.update([11]);
            state.update(index.to_be_bytes());
            bytes(state, source);
            bytes(state, target);
            relationship(state, kind);
        }
    }
}

pub(crate) fn validate_generated_candidate(
    request: &GenerativePresenterRequest,
    candidate: &GeneratedManifestationCandidate,
) -> Result<(), GenerativePresenterRefusal> {
    for identity in [
        &candidate.candidate_identity,
        &candidate.presenter_implementation_identity,
        &candidate.provider_identity,
        &candidate.model_identity,
        &candidate.generation_run_identity,
        &candidate.mask_contract_revision,
    ] {
        super::generative_presenter::validate_identity(identity)?;
    }
    if candidate.request_identity != request.request_identity {
        return Err(GenerativePresenterRefusal::RequestMismatch);
    }
    if candidate.source_presentation_identity != request.semantic_data.source_presentation_identity
        || candidate.source_presentation_revision
            != request.semantic_data.source_presentation_revision
    {
        return Err(GenerativePresenterRefusal::SourcePresentationMismatch);
    }
    if candidate.template_contract_revision != request.policy.template_contract_revision {
        return Err(GenerativePresenterRefusal::TemplateMismatch);
    }
    if candidate.content.len() > MAX_GENERATED_CONTENT_SEGMENTS {
        return Err(GenerativePresenterRefusal::TooManyContentSegments);
    }
    let mut bytes = 0usize;
    for segment in &candidate.content {
        if segment.bytes.is_empty() {
            return Err(GenerativePresenterRefusal::EmptyGeneratedContent);
        }
        bytes = bytes
            .checked_add(segment.bytes.len())
            .ok_or(GenerativePresenterRefusal::OutputBoundExceeded)?;
    }
    if bytes > request.bounds.maximum_output_bytes as usize {
        return Err(GenerativePresenterRefusal::OutputBoundExceeded);
    }
    if candidate.affordances.len() > MAX_GENERATED_AFFORDANCES {
        return Err(GenerativePresenterRefusal::TooManyAffordances);
    }
    if candidate.correlations.len() > MAX_GENERATED_CORRELATIONS {
        return Err(GenerativePresenterRefusal::TooManyCorrelations);
    }
    let produced = matches!(
        candidate.disposition,
        GeneratedManifestationDisposition::Produced | GeneratedManifestationDisposition::Truncated
    );
    if produced && candidate.content.is_empty() {
        return Err(GenerativePresenterRefusal::EmptyGeneratedContent);
    }
    if !produced
        && (!candidate.content.is_empty()
            || !candidate.affordances.is_empty()
            || !candidate.correlations.is_empty())
    {
        return Err(GenerativePresenterRefusal::OutputForTerminalDisposition);
    }
    if produced && candidate.correlations.is_empty() {
        return Err(GenerativePresenterRefusal::MissingSemanticCorrelation);
    }
    for (position, correlation) in candidate.correlations.iter().enumerate() {
        if candidate.correlations[position + 1..].contains(correlation) {
            return Err(GenerativePresenterRefusal::DuplicateSemanticCorrelation);
        }
        validate_correlation(request, correlation)?;
    }
    for affordance in &candidate.affordances {
        if *affordance.source_presentation_revision()
            != request.semantic_data.source_presentation_revision
        {
            return Err(GenerativePresenterRefusal::StaleAction);
        }
        match request.semantic_data.presentation.resolve_action(
            *affordance.source_presentation_revision(),
            affordance.action_identity(),
        ) {
            Ok(_) => {}
            Err(PresentationActionRefusal::StaleRevision) => {
                return Err(GenerativePresenterRefusal::StaleAction)
            }
            Err(PresentationActionRefusal::UnknownAction) => {
                return Err(GenerativePresenterRefusal::UnknownAction)
            }
            Err(
                PresentationActionRefusal::Unavailable { .. }
                | PresentationActionRefusal::Refused { .. },
            ) => return Err(GenerativePresenterRefusal::UnavailableAction),
        }
        if !candidate.correlations.iter().any(|item| matches!(item, GeneratedSemanticCorrelation::Action { identity, .. } if identity == affordance.action_identity())) { return Err(GenerativePresenterRefusal::UncorrelatedAction); }
    }
    if candidate.candidate_identity != candidate.digest() {
        return Err(GenerativePresenterRefusal::CandidateIdentityMismatch);
    }
    Ok(())
}

pub(crate) fn validate_correlation(
    request: &GenerativePresenterRequest,
    correlation: &GeneratedSemanticCorrelation,
) -> Result<(), GenerativePresenterRefusal> {
    let p = &request.semantic_data.presentation;
    let valid = match correlation {
        GeneratedSemanticCorrelation::Subject { index, identity } => p
            .subjects
            .get(*index as usize)
            .is_some_and(|v| &v.identity == identity),
        GeneratedSemanticCorrelation::Relationship {
            index,
            source,
            target,
            kind,
        } => p
            .relationships
            .get(*index as usize)
            .is_some_and(|v| &v.source == source && &v.target == target && &v.kind == kind),
        GeneratedSemanticCorrelation::Text { index, subject } => p
            .text
            .get(*index as usize)
            .is_some_and(|v| &v.subject == subject),
        GeneratedSemanticCorrelation::Property {
            index,
            subject,
            name,
        } => p
            .properties
            .get(*index as usize)
            .is_some_and(|v| &v.subject == subject && &v.name == name),
        GeneratedSemanticCorrelation::TypedContent {
            index,
            subject,
            name,
            content_profile,
        } => p.properties.get(*index as usize).is_some_and(|v| {
            &v.subject == subject
                && &v.name == name
                && match &v.value {
                    crate::PresentationPropertyValue::Content(encoded) => {
                        conduit_core::BoundedResourceRef::validate_encoded(encoded)
                            .is_ok_and(|reference| reference.content_profile == content_profile)
                    }
                    _ => false,
                }
        }),
        GeneratedSemanticCorrelation::Composition { index, identity } => p
            .composition
            .get(*index as usize)
            .is_some_and(|v| &v.identity == identity),
        GeneratedSemanticCorrelation::Action {
            index,
            identity,
            intent,
            target,
        } => p
            .actions
            .get(*index as usize)
            .is_some_and(|v| &v.identity == identity && &v.intent == intent && &v.target == target),
        GeneratedSemanticCorrelation::ActionArgument {
            action_index,
            argument_index,
            name,
            value_kind,
        } => p.actions.get(*action_index as usize).is_some_and(|action| {
            action
                .arguments
                .get(*argument_index as usize)
                .is_some_and(|argument| {
                    &argument.name == name && argument.contract.value_kind.as_str() == value_kind
                })
        }),
        GeneratedSemanticCorrelation::Disclosure {
            index,
            subject,
            level,
        } => p
            .disclosures
            .get(*index as usize)
            .is_some_and(|v| &v.subject == subject && &v.level == level),
        GeneratedSemanticCorrelation::TemporalReference { index, identity } => p
            .temporal_references
            .get(*index as usize)
            .is_some_and(|v| &v.identity == identity),
        GeneratedSemanticCorrelation::TemporalFact {
            index,
            subject,
            reference,
            role,
        } => p
            .temporal_facts
            .get(*index as usize)
            .is_some_and(|v| &v.subject == subject && &v.reference == reference && &v.role == role),
        GeneratedSemanticCorrelation::Context {
            index,
            source,
            target,
            relationship,
        } => p
            .interaction_context
            .basis
            .get(*index as usize)
            .is_some_and(|v| {
                &v.source == source && &v.target == target && &v.relationship == relationship
            }),
    };
    if valid {
        Ok(())
    } else {
        Err(GenerativePresenterRefusal::InvalidSemanticCorrelation)
    }
}
