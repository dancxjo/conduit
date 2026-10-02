use alloc::vec::Vec;
use conduit_core::semantic_digest;

use super::*;
use crate::ModelSignature;

impl ModelRelationSignature {
    pub fn semantic_digest(&self) -> Result<[u8; 32], RelationRefusal> {
        self.validate()?;
        let variable_signature = ModelSignature::inference_only(
            self.identity.clone(),
            self.compatibility_version,
            self.variables
                .get()
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
        .map_err(|_| RelationRefusal::InvalidSignature)?;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(self.callable_signature_identity.get());
        bytes.extend_from_slice(
            &variable_signature
                .semantic_digest()
                .map_err(|_| RelationRefusal::InvalidSignature)?,
        );
        bytes.extend_from_slice(&(self.supported_queries.get().len() as u64).to_le_bytes());
        for pattern in self.supported_queries.get() {
            push_sorted_native_text(&mut bytes, pattern.evidence_variables());
            push_sorted_native_text(&mut bytes, pattern.target_variables());
            bytes.push(mode_tag(*pattern.mode()));
            push_profile(&mut bytes, pattern.result_profile());
            bytes.extend_from_slice(&pattern.maximum_work_units().to_le_bytes());
            bytes.extend_from_slice(&pattern.maximum_output_bytes().to_le_bytes());
        }
        Ok(semantic_digest("ai/model-relation-signature@1", &bytes))
    }
}

impl RelationQuery {
    pub fn semantic_digest(&self) -> Result<[u8; 32], RelationRefusal> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(self.identity.get());
        bytes.extend_from_slice(self.artifact_identity.get());
        match self.checkpoint_identity.as_ref() {
            Some(identity) => {
                bytes.push(1);
                bytes.extend_from_slice(identity.get());
            }
            None => bytes.push(0),
        }
        bytes.extend_from_slice(self.relation_signature_identity.get());
        let mut evidence = self.evidence.get().iter().collect::<Vec<_>>();
        evidence.sort_by(|left, right| left.variable.get().cmp(right.variable.get()));
        for value in evidence {
            push_text(&mut bytes, value.variable.get());
            bytes.extend_from_slice(&value.value.semantic_digest()?);
        }
        push_sorted_native_text(&mut bytes, &self.targets);
        bytes.push(mode_tag(self.mode));
        push_profile(&mut bytes, &self.requested_result);
        match &self.randomness {
            RandomnessProfile::Deterministic => bytes.push(0),
            RandomnessProfile::ExplicitSeed(payload) => {
                bytes.push(1);
                bytes.extend_from_slice(&payload.seed().to_le_bytes());
            }
            RandomnessProfile::ProviderChosen(payload) => {
                bytes.push(2);
                bytes.extend_from_slice(&payload.seed().to_le_bytes());
                push_text(&mut bytes, payload.nonce());
            }
        }
        bytes.extend_from_slice(&self.admitted_work_units.to_le_bytes());
        bytes.extend_from_slice(&self.maximum_output_bytes.to_le_bytes());
        Ok(semantic_digest("ai/relation-query@1", &bytes))
    }
}

fn push_sorted_native_text(output: &mut Vec<u8>, values: &RelationVariableIdentities) {
    let mut values = values
        .get()
        .iter()
        .map(RelationVariableIdentity::get)
        .collect::<Vec<_>>();
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

fn push_profile(output: &mut Vec<u8>, value: &RelationResultProfile) {
    match value {
        RelationResultProfile::Deterministic => output.push(0),
        RelationResultProfile::Probabilistic(profile) => {
            output.push(1);
            output.extend_from_slice(&profile.maximum_samples().to_le_bytes());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn result_profile_digest_bytes_remain_exact() {
        let mut bytes = Vec::new();
        push_profile(&mut bytes, &RelationResultProfile::Deterministic);
        assert_eq!(bytes, [0]);

        bytes.clear();
        let probabilistic = RelationResultProfile::probabilistic(0x0102_0304).unwrap();
        push_profile(&mut bytes, &probabilistic);
        assert_eq!(bytes, [1, 4, 3, 2, 1]);
    }
}
