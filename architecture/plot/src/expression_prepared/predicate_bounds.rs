//! Preparation-only bounds for predicates retained by this exact evaluator.
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExpressionValueContractBounds {
    pub maximum_constraints: u64,
    /// Replay containers and buffers; contract-owned heap payload is separate.
    pub prepared_near_replay_bytes: u64,
    pub maximum_numerical_work: u64,
    pub retained_near_schema_bytes: u64,
    pub canonical_contract_bytes: u64,
    pub maximum_input_bytes: u64,
    pub maximum_output_bytes: u64,
}
impl PreparedPortableExpressionEvaluator {
    /// Numerical work is the sum of the validators' independent finite meters.
    /// Byte ceilings and constraint count additionally bound framing and scalar
    /// predicates. Schema/canonical payload figures exclude allocator bookkeeping
    /// and the separately prepared evaluator tree, input and output storage.
    pub fn value_contract_bounds(&self) -> Option<ExpressionValueContractBounds> {
        if self.input_value_contract.is_none() && self.output_value_contract.is_none() {
            return None;
        }
        let mut bounds = ExpressionValueContractBounds {
            maximum_constraints: 0,
            prepared_near_replay_bytes: 0,
            maximum_numerical_work: 0,
            retained_near_schema_bytes: 0,
            canonical_contract_bytes: 0,
            maximum_input_bytes: self
                .input_value_contract
                .as_ref()
                .map_or(0, |c| u64::from(c.contract().maximum_bytes)),
            maximum_output_bytes: self
                .output_value_contract
                .as_ref()
                .map_or(0, |c| u64::from(c.contract().maximum_bytes)),
        };
        for contract in [&self.input_value_contract, &self.output_value_contract]
            .into_iter()
            .flatten()
        {
            let replay = contract.replay_bounds();
            bounds.prepared_near_replay_bytes += replay.map_or(0, |r| r.0);
            let contract = contract.contract();
            bounds.maximum_constraints += contract.constraints.len() as u64;
            bounds.canonical_contract_bytes += contract.identity_bytes().len() as u64;
            for constraint in &contract.constraints {
                if let conduit_core::ValueConstraint::NearQuantity { schema, .. } = constraint {
                    bounds.maximum_numerical_work +=
                        replay.map_or(conduit_core::NEAR_VALUE_CONSTRAINT_WORK, |r| r.1);
                    bounds.retained_near_schema_bytes += schema.capacity() as u64;
                }
            }
        }
        Some(bounds)
    }
}
