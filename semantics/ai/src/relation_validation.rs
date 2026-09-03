use alloc::{string::String, vec::Vec};
use conduit_data::TensorValue;

use super::*;
use crate::{
    ModelArtifact, ModelDimensionConstraint, ModelOperation, ModelPortConstraint,
    ModelPortPresence, ModelSignature, ProbabilisticDisposition,
};

impl ModelRelationSignature {
    pub fn validate(&self) -> Result<(), RelationRefusal> {
        text(&self.identity)?;
        nonzero(self.callable_signature_identity)?;
        if self.compatibility_version == 0 || self.variables.is_empty() {
            return Err(RelationRefusal::InvalidSignature);
        }
        if self.variables.len() > MAXIMUM_RELATION_VARIABLES {
            return Err(RelationRefusal::TooManyVariables);
        }
        for variable in &self.variables {
            text(&variable.identity)?;
            text(&variable.semantic_role)?;
            validate_constraint(&variable.value)?;
        }
        ModelSignature {
            identity: self.identity.clone(),
            compatibility_version: self.compatibility_version,
            operations: alloc::vec![ModelOperation::Infer],
            inputs: self
                .variables
                .iter()
                .map(|variable| ModelPortConstraint {
                    identity: variable.identity.clone(),
                    semantic_kind: variable.semantic_role.clone(),
                    presence: ModelPortPresence::Optional,
                    value: variable.value.clone(),
                })
                .collect(),
            outputs: Vec::new(),
        }
        .validate()
        .map_err(|_| RelationRefusal::InvalidSignature)?;
        if duplicate(self.variables.iter().map(|value| &value.identity)) {
            return Err(RelationRefusal::DuplicateVariable);
        }
        if self.supported_queries.is_empty() {
            return Err(RelationRefusal::InvalidPattern);
        }
        if self.supported_queries.len() > MAXIMUM_RELATION_PATTERNS {
            return Err(RelationRefusal::TooManyPatterns);
        }
        for pattern in &self.supported_queries {
            pattern.validate_for(self)?;
        }
        if self
            .supported_queries
            .iter()
            .enumerate()
            .any(|(index, pattern)| {
                self.supported_queries[index + 1..]
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
            .iter()
            .find(|pattern| pattern.matches(query))
            .ok_or(RelationRefusal::UnsupportedQuery)?;
        if query.admitted_work_units > pattern.maximum_work_units
            || query.maximum_output_bytes > pattern.maximum_output_bytes
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
            .iter()
            .map(|evidence| Ok((evidence.variable.clone(), evidence.value.semantic_digest()?)))
            .collect::<Result<Vec<_>, RelationRefusal>>()?;
        Ok(RelationQueryOutcome::Completed(Box::new(RelationReceipt {
            query_identity: query.identity,
            query_descriptor_identity: query.semantic_digest()?,
            artifact_identity: query.artifact_identity,
            checkpoint_identity: query.checkpoint_identity,
            relation_signature_identity: query.relation_signature_identity,
            evidence_identities,
            targets: query.targets.clone(),
            mode: query.mode,
            requested_result: query.requested_result,
            randomness: query.randomness.clone(),
            admitted_work_units: query.admitted_work_units,
            consumed_work_units: candidate.consumed_work_units,
            output_identities: candidate
                .outputs
                .iter()
                .map(|output| (output.target_variable.clone(), output.value_identity))
                .collect(),
            realization: candidate.realization,
        })))
    }

    fn variable(&self, identity: &str) -> Option<&RelationVariable> {
        self.variables
            .iter()
            .find(|value| value.identity == identity)
    }
}

impl SupportedRelationQuery {
    fn validate_for(&self, signature: &ModelRelationSignature) -> Result<(), RelationRefusal> {
        if self.evidence_variables.is_empty()
            || self.target_variables.is_empty()
            || self.evidence_variables.len() > MAXIMUM_RELATION_VALUES
            || self.target_variables.len() > MAXIMUM_RELATION_VALUES
            || self.maximum_work_units == 0
            || self.maximum_output_bytes == 0
            || duplicate(self.evidence_variables.iter())
            || duplicate(self.target_variables.iter())
            || self
                .evidence_variables
                .iter()
                .any(|value| self.target_variables.contains(value))
        {
            return Err(RelationRefusal::InvalidPattern);
        }
        for identity in self.evidence_variables.iter().chain(&self.target_variables) {
            text(identity)?;
            if signature.variable(identity).is_none() {
                return Err(RelationRefusal::UnknownVariable);
            }
        }
        if matches!(
            self.result_profile,
            RelationResultProfile::Probabilistic { maximum_samples: 0 }
        ) {
            return Err(RelationRefusal::InvalidPattern);
        }
        Ok(())
    }

    fn same_query(&self, other: &Self) -> bool {
        self.mode == other.mode
            && same_set(&self.evidence_variables, &other.evidence_variables)
            && same_set(&self.target_variables, &other.target_variables)
    }

    fn matches(&self, query: &RelationQuery) -> bool {
        self.mode == query.mode
            && self.result_profile == query.requested_result
            && same_set(
                &self.evidence_variables,
                &query
                    .evidence
                    .iter()
                    .map(|value| value.variable.clone())
                    .collect::<Vec<_>>(),
            )
            && same_set(&self.target_variables, &query.targets)
    }
}

impl RelationQuery {
    fn validate_for(
        &self,
        signature: &ModelRelationSignature,
        artifact: &ModelArtifact,
    ) -> Result<(), RelationRefusal> {
        nonzero(self.identity)?;
        if artifact.signature_identity != signature.callable_signature_identity
            || self.artifact_identity != artifact.content_identity()
            || self.relation_signature_identity != signature.semantic_digest()?
            || self.checkpoint_identity == Some([0; 32])
        {
            return Err(RelationRefusal::ArtifactMismatch);
        }
        if self.evidence.is_empty() || self.targets.is_empty() {
            return Err(RelationRefusal::UnsupportedQuery);
        }
        if duplicate(self.evidence.iter().map(|value| &value.variable)) {
            return Err(RelationRefusal::DuplicateEvidence);
        }
        if duplicate(self.targets.iter()) {
            return Err(RelationRefusal::DuplicateTarget);
        }
        for evidence in &self.evidence {
            let variable = signature
                .variable(&evidence.variable)
                .ok_or(RelationRefusal::UnknownVariable)?;
            evidence.value.validate_against(&variable.value)?;
        }
        for target in &self.targets {
            if signature.variable(target).is_none() {
                return Err(RelationRefusal::UnknownVariable);
            }
        }
        match (&self.requested_result, &self.randomness) {
            (RelationResultProfile::Deterministic, RandomnessProfile::Deterministic) => {}
            (RelationResultProfile::Probabilistic { maximum_samples }, _)
                if *maximum_samples > 0
                    && !matches!(self.randomness, RandomnessProfile::Deterministic) => {}
            _ => return Err(RelationRefusal::DeterminismMismatch),
        }
        if self.admitted_work_units == 0 || self.maximum_output_bytes == 0 {
            return Err(RelationRefusal::WorkBoundExceeded);
        }
        Ok(())
    }
}

impl RelationCandidate {
    fn validate_for(&self, query: &RelationQuery) -> Result<(), RelationRefusal> {
        if self.outputs.len() != query.targets.len()
            || duplicate(self.outputs.iter().map(|value| &value.target_variable))
            || !same_set(
                &self
                    .outputs
                    .iter()
                    .map(|value| value.target_variable.clone())
                    .collect::<Vec<_>>(),
                &query.targets,
            )
        {
            return Err(RelationRefusal::InvalidResult);
        }
        for output in &self.outputs {
            nonzero(output.value_identity)?;
            match (&query.requested_result, &output.disposition) {
                (RelationResultProfile::Deterministic, ProbabilisticDisposition::Exact)
                    if output.sample_count == 1 => {}
                (RelationResultProfile::Probabilistic { maximum_samples }, _)
                    if output.sample_count > 0 && output.sample_count <= *maximum_samples => {}
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
            Self::Tensor(value) => Ok(value.content_digest),
            Self::SampledSignal(value) => value
                .semantic_digest()
                .map_err(|_| RelationRefusal::InvalidValue),
        }
    }

    fn validate_against(&self, constraint: &ModelValueConstraint) -> Result<(), RelationRefusal> {
        match (self, constraint) {
            (Self::Tensor(value), ModelValueConstraint::Tensor(constraint)) => {
                validate_tensor(value, constraint)
            }
            (Self::SampledSignal(value), ModelValueConstraint::SampledSignal(constraint)) => {
                value
                    .validate()
                    .map_err(|_| RelationRefusal::InvalidValue)?;
                validate_tensor(&value.samples, constraint)
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
    if !constraint.elements.contains(&value.element)
        || value.dimensions.len() != constraint.axes.len()
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
        .zip(&constraint.axes)
    {
        let valid = match expected.dimension {
            ModelDimensionConstraint::Fixed(value) => *dimension == value,
            ModelDimensionConstraint::Bounded { minimum, maximum } => {
                *dimension >= minimum && *dimension <= maximum
            }
        };
        if !valid || axis.role != expected.role {
            return Err(RelationRefusal::ShapeMismatch);
        }
    }
    Ok(())
}

fn validate_constraint(value: &ModelValueConstraint) -> Result<(), RelationRefusal> {
    let constraint = match value {
        ModelValueConstraint::Tensor(value)
        | ModelValueConstraint::SampledSignal(value)
        | ModelValueConstraint::ProbabilisticTensor(value)
        | ModelValueConstraint::ProbabilisticSignal(value) => value,
    };
    if constraint.elements.is_empty() || constraint.axes.is_empty() || constraint.maximum_bytes == 0
    {
        Err(RelationRefusal::InvalidSignature)
    } else {
        Ok(())
    }
}

fn same_set(left: &[String], right: &[String]) -> bool {
    left.len() == right.len() && left.iter().all(|value| right.contains(value))
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
