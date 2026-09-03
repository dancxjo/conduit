use alloc::{string::String, vec::Vec};
use conduit_core::semantic_digest;

use super::*;
use crate::{ModelOperation, ModelPortConstraint, ModelPortPresence, ModelSignature};

impl ModelRelationSignature {
    pub fn semantic_digest(&self) -> Result<[u8; 32], RelationRefusal> {
        self.validate()?;
        let variable_signature = ModelSignature {
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
        };
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.callable_signature_identity);
        bytes.extend_from_slice(
            &variable_signature
                .semantic_digest()
                .map_err(|_| RelationRefusal::InvalidSignature)?,
        );
        bytes.extend_from_slice(&(self.supported_queries.len() as u64).to_le_bytes());
        for pattern in &self.supported_queries {
            push_sorted_text(&mut bytes, &pattern.evidence_variables);
            push_sorted_text(&mut bytes, &pattern.target_variables);
            bytes.push(mode_tag(pattern.mode));
            push_profile(&mut bytes, pattern.result_profile);
            bytes.extend_from_slice(&pattern.maximum_work_units.to_le_bytes());
            bytes.extend_from_slice(&pattern.maximum_output_bytes.to_le_bytes());
        }
        Ok(semantic_digest("ai/model-relation-signature@1", &bytes))
    }
}

impl RelationQuery {
    pub fn semantic_digest(&self) -> Result<[u8; 32], RelationRefusal> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.identity);
        bytes.extend_from_slice(&self.artifact_identity);
        match self.checkpoint_identity {
            Some(identity) => {
                bytes.push(1);
                bytes.extend_from_slice(&identity);
            }
            None => bytes.push(0),
        }
        bytes.extend_from_slice(&self.relation_signature_identity);
        let mut evidence = self.evidence.iter().collect::<Vec<_>>();
        evidence.sort_by(|left, right| left.variable.cmp(&right.variable));
        for value in evidence {
            push_text(&mut bytes, &value.variable);
            bytes.extend_from_slice(&value.value.semantic_digest()?);
        }
        push_sorted_text(&mut bytes, &self.targets);
        bytes.push(mode_tag(self.mode));
        push_profile(&mut bytes, self.requested_result);
        match &self.randomness {
            RandomnessProfile::Deterministic => bytes.push(0),
            RandomnessProfile::ExplicitSeed(seed) => {
                bytes.push(1);
                bytes.extend_from_slice(&seed.to_le_bytes());
            }
            RandomnessProfile::ProviderChosen { seed, nonce } => {
                bytes.push(2);
                bytes.extend_from_slice(&seed.to_le_bytes());
                push_text(&mut bytes, nonce);
            }
        }
        bytes.extend_from_slice(&self.admitted_work_units.to_le_bytes());
        bytes.extend_from_slice(&self.maximum_output_bytes.to_le_bytes());
        Ok(semantic_digest("ai/relation-query@1", &bytes))
    }
}

fn push_sorted_text(output: &mut Vec<u8>, values: &[String]) {
    let mut values = values.iter().collect::<Vec<_>>();
    values.sort();
    output.extend_from_slice(&(values.len() as u64).to_le_bytes());
    for value in values {
        push_text(output, value);
    }
}

fn push_text(output: &mut Vec<u8>, value: &str) {
    output.extend_from_slice(&(value.len() as u64).to_le_bytes());
    output.extend_from_slice(value.as_bytes());
}

fn mode_tag(value: RelationQueryMode) -> u8 {
    match value {
        RelationQueryMode::InferPosterior => 0,
        RelationQueryMode::SampleConditional => 1,
        RelationQueryMode::Reconstruct => 2,
        RelationQueryMode::EncodeLatent => 3,
        RelationQueryMode::DecodeGenerate => 4,
        RelationQueryMode::LogProbability => 5,
    }
}

fn push_profile(output: &mut Vec<u8>, value: RelationResultProfile) {
    match value {
        RelationResultProfile::Deterministic => output.push(0),
        RelationResultProfile::Probabilistic { maximum_samples } => {
            output.push(1);
            output.extend_from_slice(&maximum_samples.to_le_bytes());
        }
    }
}
