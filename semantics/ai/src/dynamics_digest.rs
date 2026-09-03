use alloc::vec::Vec;
use conduit_core::{semantic_digest, QuantityUnit};

use crate::{DynamicsProfile, DynamicsRefusal, IntegrateContract};

impl IntegrateContract {
    pub fn semantic_digest(&self) -> Result<[u8; 32], DynamicsRefusal> {
        self.validate()?;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.identity);
        bytes.extend_from_slice(&self.vector_field_artifact_identity);
        bytes.extend_from_slice(&self.interval.start.to_le_bytes());
        bytes.extend_from_slice(&self.interval.end.to_le_bytes());
        bytes.push(unit_tag(self.interval.unit));
        push_text(&mut bytes, &self.sampling.clock_identity);
        bytes.extend_from_slice(&(self.sampling.coordinates.len() as u64).to_le_bytes());
        for coordinate in &self.sampling.coordinates {
            bytes.extend_from_slice(&coordinate.to_le_bytes());
        }
        match &self.profile {
            DynamicsProfile::DeterministicOde => bytes.push(0),
            DynamicsProfile::Stochastic { .. } => unreachable!("validation refuses SDE profiles"),
        }
        bytes.extend_from_slice(&self.accuracy.absolute_tolerance_millionths.to_le_bytes());
        bytes.extend_from_slice(&self.accuracy.relative_tolerance_millionths.to_le_bytes());
        bytes.extend_from_slice(
            &self
                .accuracy
                .maximum_estimated_error_millionths
                .to_le_bytes(),
        );
        bytes.extend_from_slice(&self.resources.maximum_state_bytes.to_le_bytes());
        bytes.extend_from_slice(&self.resources.maximum_context_bytes.to_le_bytes());
        bytes.extend_from_slice(&self.resources.maximum_output_samples.to_le_bytes());
        bytes.extend_from_slice(&self.resources.maximum_output_bytes.to_le_bytes());
        bytes.extend_from_slice(&self.resources.maximum_internal_steps.to_le_bytes());
        bytes.extend_from_slice(&self.resources.maximum_function_evaluations.to_le_bytes());
        bytes.extend_from_slice(&self.resources.maximum_work_units.to_le_bytes());
        bytes.extend_from_slice(&self.resources.memory_ceiling_bytes.to_le_bytes());
        Ok(semantic_digest("ai/integrate-contract@1", &bytes))
    }
}

fn push_text(output: &mut Vec<u8>, value: &str) {
    output.extend_from_slice(&(value.len() as u64).to_le_bytes());
    output.extend_from_slice(value.as_bytes());
}

fn unit_tag(unit: QuantityUnit) -> u8 {
    match unit {
        QuantityUnit::Second => 0,
        QuantityUnit::Millisecond => 1,
        QuantityUnit::Microsecond => 2,
        QuantityUnit::Nanosecond => 3,
        _ => unreachable!("validation admits only temporal units"),
    }
}
