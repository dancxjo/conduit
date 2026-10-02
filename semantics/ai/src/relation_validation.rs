use alloc::{string::String, vec::Vec};
use conduit_data::TensorValue;

use super::*;
use crate::{ModelArtifact, ModelDimensionConstraint, ModelSignature, ProbabilisticDisposition};

impl ModelRelationSignature {
    pub fn validate(&self) -> Result<(), RelationRefusal> {
        text(&self.identity)?;
        nonzero(*self.callable_signature_identity.get())?;
        let variables = self.variables.get().as_slice();
        let supported_queries = self.supported_queries.get().as_slice();
        ModelSignature::inference_only(
            self.identity.clone(),
            self.compatibility_version,
            variables
                .iter()
                .map(|variable| {
                    (
                        variable.identity.get().clone(),
                        variable.semantic_role.get().clone(),
                        variable.value.clone(),
                    )
                })
                .collect(),
        )
        .map_err(|_| RelationRefusal::InvalidSignature)?
        .validate()
        .map_err(|_| RelationRefusal::InvalidSignature)?;
        if duplicate(variables.iter().map(|value| value.identity.get())) {
            return Err(RelationRefusal::DuplicateVariable);
        }
        for pattern in supported_queries {
            pattern.validate_for(self)?;
        }
        if self
            .supported_queries
            .get()
            .iter()
            .enumerate()
            .any(|(index, pattern)| {
                supported_queries[index + 1..]
                    .iter()
                    .any(|other| pattern.same_query(other))
            })
        {
            return Err(RelationRefusal::DuplicatePattern);
        }
        Ok(())
    }

    pub fn realize(
        &self,
        artifact: &ModelArtifact,
        query: &RelationQuery,
        terminal: HostRelationTerminal,
    ) -> Result<RelationQueryOutcome, RelationRefusal> {
        self.validate()?;
        query.validate_for(self, artifact)?;
        let pattern = self
            .supported_queries
            .get()
            .iter()
            .find(|pattern| pattern.matches(query))
            .ok_or(RelationRefusal::UnsupportedQuery)?;
        if query.admitted_work_units > *pattern.maximum_work_units()
            || query.maximum_output_bytes > *pattern.maximum_output_bytes()
        {
            return Err(RelationRefusal::WorkBoundExceeded);
        }
        let candidate = match terminal {
            HostRelationTerminal::NoResult(terminal) => {
                return Ok(RelationQueryOutcome::NoResult(terminal))
            }
            HostRelationTerminal::Candidate(candidate) => candidate,
        };
        candidate.validate_for(query)?;
        let evidence_identities = query
            .evidence
            .get()
            .iter()
            .map(|evidence| {
                Ok((
                    evidence.variable.get().clone(),
                    evidence.value.semantic_digest()?,
                ))
            })
            .collect::<Result<Vec<_>, RelationRefusal>>()?;
        Ok(RelationQueryOutcome::Completed(Box::new(RelationReceipt {
            query_identity: *query.identity.get(),
            query_descriptor_identity: query.semantic_digest()?,
            artifact_identity: *query.artifact_identity.get(),
            checkpoint_identity: query.checkpoint_identity.as_ref().map(|value| *value.get()),
            relation_signature_identity: *query.relation_signature_identity.get(),
            evidence_identities,
            targets: query
                .targets
                .get()
                .iter()
                .map(|value| value.get().clone())
                .collect(),
            mode: query.mode,
            requested_result: query.requested_result.clone(),
            randomness: query.randomness.clone(),
            admitted_work_units: query.admitted_work_units,
            consumed_work_units: candidate.consumed_work_units,
            output_identities: candidate
                .outputs
                .iter()
                .map(|output| {
                    (
                        output.target_variable().get().clone(),
                        *output.value_identity().get(),
                    )
                })
                .collect(),
            realization: candidate.realization,
        })))
    }

    fn variable(&self, identity: &str) -> Option<&RelationVariable> {
        self.variables
            .get()
            .iter()
            .find(|value| value.identity.get() == identity)
    }
}

impl SupportedRelationQuery {
    fn validate_for(&self, signature: &ModelRelationSignature) -> Result<(), RelationRefusal> {
        let evidence = self.evidence_variables().get().as_slice();
        let targets = self.target_variables().get().as_slice();
        if duplicate_native(evidence)
            || duplicate_native(targets)
            || evidence.iter().any(|value| targets.contains(value))
        {
            return Err(RelationRefusal::InvalidPattern);
        }
        for identity in evidence.iter().chain(targets) {
            if signature.variable(identity.get()).is_none() {
                return Err(RelationRefusal::UnknownVariable);
            }
        }
        if matches!(
            self.result_profile(),
            RelationResultProfile::Probabilistic(profile) if *profile.maximum_samples() == 0
        ) {
            return Err(RelationRefusal::InvalidPattern);
        }
        Ok(())
    }

    fn same_query(&self, other: &Self) -> bool {
        self.mode() == other.mode()
            && same_native_set(
                self.evidence_variables().get().as_slice(),
                other.evidence_variables().get().as_slice(),
            )
            && same_native_set(
                self.target_variables().get().as_slice(),
                other.target_variables().get().as_slice(),
            )
    }

    fn matches(&self, query: &RelationQuery) -> bool {
        self.mode() == &query.mode
            && self.result_profile() == &query.requested_result
            && native_matches_strings(
                self.evidence_variables().get().as_slice(),
                &query
                    .evidence
                    .get()
                    .iter()
                    .map(|value| value.variable.get().clone())
                    .collect::<Vec<_>>(),
            )
            && same_native_set(
                self.target_variables().get().as_slice(),
                query.targets.get().as_slice(),
            )
    }
}

impl RelationQuery {
    fn validate_for(
        &self,
        signature: &ModelRelationSignature,
        artifact: &ModelArtifact,
    ) -> Result<(), RelationRefusal> {
        if artifact.signature_identity != *signature.callable_signature_identity.get()
            || *self.artifact_identity.get() != artifact.content_identity()
            || *self.relation_signature_identity.get() != signature.semantic_digest()?
            || self
                .checkpoint_identity
                .as_ref()
                .is_some_and(|value| value.get() == &[0; 32])
        {
            return Err(RelationRefusal::ArtifactMismatch);
        }
        let evidence_values = self.evidence.get().as_slice();
        let targets = self.targets.get().as_slice();
        if duplicate_native(
            &evidence_values
                .iter()
                .map(|value| value.variable.clone())
                .collect::<Vec<_>>(),
        ) {
            return Err(RelationRefusal::DuplicateEvidence);
        }
        if duplicate_native(targets) {
            return Err(RelationRefusal::DuplicateTarget);
        }
        for evidence in evidence_values {
            let variable = signature
                .variable(evidence.variable.get())
                .ok_or(RelationRefusal::UnknownVariable)?;
            evidence.value.validate_against(&variable.value)?;
        }
        for target in targets {
            if signature.variable(target.get()).is_none() {
                return Err(RelationRefusal::UnknownVariable);
            }
        }
        match (&self.requested_result, &self.randomness) {
            (RelationResultProfile::Deterministic, RandomnessProfile::Deterministic) => {}
            (RelationResultProfile::Probabilistic(profile), _)
                if *profile.maximum_samples() > 0
                    && !matches!(self.randomness, RandomnessProfile::Deterministic) => {}
            _ => return Err(RelationRefusal::DeterminismMismatch),
        }
        Ok(())
    }
}

impl RelationCandidate {
    fn validate_for(&self, query: &RelationQuery) -> Result<(), RelationRefusal> {
        let output_targets = self
            .outputs
            .iter()
            .map(|value| value.target_variable().get().clone())
            .collect::<Vec<_>>();
        let targets = query.targets.get().as_slice();
        if self.outputs.len() != targets.len()
            || duplicate(output_targets.iter())
            || !native_matches_strings(targets, &output_targets)
        {
            return Err(RelationRefusal::InvalidResult);
        }
        for output in &self.outputs {
            nonzero(*output.value_identity().get())?;
            match (&query.requested_result, output.disposition()) {
                (RelationResultProfile::Deterministic, ProbabilisticDisposition::Exact)
                    if *output.sample_count() == 1 => {}
                (RelationResultProfile::Probabilistic(profile), _)
                    if *output.sample_count() > 0
                        && *output.sample_count() <= *profile.maximum_samples() => {}
                _ => return Err(RelationRefusal::DeterminismMismatch),
            }
        }
        if self.consumed_work_units == 0 || self.consumed_work_units > query.admitted_work_units {
            return Err(RelationRefusal::WorkBoundExceeded);
        }
        if self.encoded_output_bytes == 0 || self.encoded_output_bytes > query.maximum_output_bytes
        {
            return Err(RelationRefusal::OutputBoundExceeded);
        }
        for value in [
            &self.realization.implementation_identity,
            &self.realization.runtime_name,
            &self.realization.runtime_version,
            &self.realization.runtime_build_identity,
            &self.realization.device_profile,
        ] {
            text(value).map_err(|_| RelationRefusal::InvalidRealization)?;
        }
        Ok(())
    }
}

impl RelationValue {
    pub(super) fn semantic_digest(&self) -> Result<[u8; 32], RelationRefusal> {
        match self {
            Self::Tensor(value) => Ok(value.value().content_digest),
            Self::SampledSignal(value) => value
                .value()
                .semantic_digest()
                .map_err(|_| RelationRefusal::InvalidValue),
        }
    }

    fn validate_against(&self, constraint: &ModelValueConstraint) -> Result<(), RelationRefusal> {
        match (self, constraint) {
            (Self::Tensor(value), ModelValueConstraint::Tensor(constraint)) => {
                validate_tensor(value.value(), constraint.constraint())
            }
            (Self::SampledSignal(value), ModelValueConstraint::SampledSignal(constraint)) => {
                value
                    .value()
                    .validate()
                    .map_err(|_| RelationRefusal::InvalidValue)?;
                validate_tensor(&value.value().samples, constraint.constraint())
            }
            _ => Err(RelationRefusal::ShapeMismatch),
        }
    }
}

fn validate_tensor(
    value: &TensorValue,
    constraint: &crate::ModelTensorConstraint,
) -> Result<(), RelationRefusal> {
    value
        .validate()
        .map_err(|_| RelationRefusal::InvalidValue)?;
    if !constraint.elements.get().contains(&value.element)
        || value.dimensions.len() != constraint.axes.get().len()
        || value
            .byte_count()
            .map_err(|_| RelationRefusal::InvalidValue)?
            > constraint.maximum_bytes
    {
        return Err(RelationRefusal::ShapeMismatch);
    }
    for ((dimension, axis), expected) in value
        .dimensions
        .iter()
        .zip(&value.axes)
        .zip(constraint.axes.get())
    {
        let valid = match &expected.dimension {
            ModelDimensionConstraint::Fixed(value) => dimension == value.value(),
            ModelDimensionConstraint::Bounded(value) => {
                dimension >= value.minimum() && dimension <= value.maximum()
            }
        };
        if !valid || axis.role != expected.role {
            return Err(RelationRefusal::ShapeMismatch);
        }
    }
    Ok(())
}

fn same_native_set(left: &[RelationVariableIdentity], right: &[RelationVariableIdentity]) -> bool {
    left.len() == right.len() && left.iter().all(|value| right.contains(value))
}

fn native_matches_strings(left: &[RelationVariableIdentity], right: &[String]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .all(|value| right.iter().any(|candidate| candidate == value.get()))
}

fn duplicate_native(values: &[RelationVariableIdentity]) -> bool {
    values
        .iter()
        .enumerate()
        .any(|(index, value)| values[index + 1..].contains(value))
}

fn duplicate<'a>(values: impl Iterator<Item = &'a String>) -> bool {
    let values = values.collect::<Vec<_>>();
    values
        .iter()
        .enumerate()
        .any(|(index, value)| values[index + 1..].contains(value))
}

fn text(value: &str) -> Result<(), RelationRefusal> {
    if value.is_empty() || value.len() > 128 {
        Err(RelationRefusal::MissingIdentity)
    } else {
        Ok(())
    }
}

fn nonzero(value: [u8; 32]) -> Result<(), RelationRefusal> {
    if value == [0; 32] {
        Err(RelationRefusal::MissingIdentity)
    } else {
        Ok(())
    }
}
