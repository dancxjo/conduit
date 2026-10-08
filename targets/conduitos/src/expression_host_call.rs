//! Exact checked expression execution under the ordinary native Host Call boundary.
use alloc::{format, vec, vec::Vec};
use conduit_core::*;
use conduit_kernel::{HostCallId, NodeId, RequestId};
use conduit_plan_lowering::lowering::LoweredPlanFragment;
use conduit_plot::{PortableExpressionProgram, PreparedPortableExpressionEvaluator};

pub const IMPLEMENTATION: &str = "conduitos/kernel-pure-expression@1";
pub const PROFILE: &str = "conduitos/pure-expression-cooperative-bounded@1";
pub const ARTIFACT: &str = "conduit-plot/pure-expression@1";
pub const CALL: &str = "conduit.host/pure-expression@1";

/// Pure computation offers no machine resource or ambient authority.
pub fn offer(
    program: &PortableExpressionProgram,
    temporal: PortTemporal,
) -> Result<CapabilityOffer, StructuredInfoRefusal> {
    let contract = conduit_semantic_catalog::pure_expression_contract(program, temporal)?;
    let kind = contract.kind_id.clone();
    let input_bytes = program
        .maximum_prepared_input_bytes()
        .map_err(|_| StructuredInfoRefusal::MalformedCanonicalEncoding)?;
    let output_bytes = program
        .maximum_prepared_output_bytes()
        .map_err(|_| StructuredInfoRefusal::MalformedCanonicalEncoding)?;
    // The native one-value operation needs only its exact frame envelope.
    // Advertising the generic Kind ceiling would inflate every kernel slot.
    let mut limits = contract.limits.clone();
    limits.max_queue_bytes = limits
        .max_queue_bytes
        .min(input_bytes.max(output_bytes).max(1));
    Ok(BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from(format!("conduitos/{}", kind.as_str())),
            execution_profile_id: ExecutionProfileId::from(PROFILE),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from(ARTIFACT),
            host_calls: vec![HostCallRequirement {
                contract_id: HostCallContractId::from(CALL),
                target_kind: Some(kind),
                maximum_in_flight: 1,
                maximum_input_bytes: input_bytes,
                maximum_output_bytes: output_bytes,
            }],
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .narrow_capacity(limits)
    .map_err(|_| StructuredInfoRefusal::MalformedCanonicalEncoding)?
    .build())
}

#[derive(Debug, PartialEq, Eq)]
pub enum ExpressionCallRefusal {
    WrongBinding,
    InvalidProgram,
    InvalidInput,
    StaleRequest,
    SequenceExhausted,
    Cancelled,
    Evaluation(conduit_plot::PortableExpressionEvaluationRefusal),
}

pub struct ExpressionHostCall {
    evaluator: PreparedPortableExpressionEvaluator,
    node: NodeId,
    next_request: u32,
    maximum_input_bytes: u32,
    cancelled: bool,
}

impl ExpressionHostCall {
    /// Validate selected realization and numeric routing before allocating execution storage.
    pub fn prepare(
        fragment: &PlanFragment,
        lowered: &LoweredPlanFragment,
        active: &ActivePlayIdentity,
        placement: &PlacementId,
    ) -> Result<Self, ExpressionCallRefusal> {
        PreparedExpressionFragment::prepare(fragment, lowered, active)?.owner(placement)
    }

    /// Evaluate once using storage admitted at preparation; never retry a failed program.
    pub fn invoke(
        &mut self,
        node: NodeId,
        call: HostCallId,
        request: RequestId,
        input: &[u8],
    ) -> Result<&[u8], ExpressionCallRefusal> {
        use ExpressionCallRefusal as Refusal;
        if node != self.node || call != HostCallId(0) {
            return Err(Refusal::WrongBinding);
        }
        if self.cancelled {
            return Err(Refusal::Cancelled);
        }
        if request != RequestId(self.next_request) {
            return Err(Refusal::StaleRequest);
        }
        if input.len() > self.maximum_input_bytes as usize {
            return Err(Refusal::InvalidInput);
        }
        self.next_request = self
            .next_request
            .checked_add(1)
            .ok_or(Refusal::SequenceExhausted)?;
        self.evaluator.evaluate(input).map_err(Refusal::Evaluation)
    }

    pub fn cancel(&mut self, node: NodeId, call: HostCallId) -> Result<(), ExpressionCallRefusal> {
        if node != self.node || call != HostCallId(0) {
            return Err(ExpressionCallRefusal::WrongBinding);
        }
        self.cancelled = true;
        Ok(())
    }
}

fn prepared_program(
    gear: &PlannedGear,
) -> Result<PortableExpressionProgram, ExpressionCallRefusal> {
    use ExpressionCallRefusal as Refusal;
    let [entry] = gear.configuration.as_slice() else {
        return Err(Refusal::InvalidProgram);
    };
    let ("program", ConfigurationValue::Text(encoded)) = (entry.key.as_str(), &entry.value) else {
        return Err(Refusal::InvalidProgram);
    };
    let program = PortableExpressionProgram::from_canonical_hex(encoded)
        .map_err(|_| Refusal::InvalidProgram)?;
    let temporal = gear.inputs.first().ok_or(Refusal::WrongBinding)?.temporal;
    let expected = offer(&program, temporal).map_err(|_| Refusal::InvalidProgram)?;
    if gear.kind_id != expected.kind_id
        || gear.kind_contract_revision != expected.kind_contract_revision
        || gear.capability_id != expected.capability_id
        || gear.execution_profile_id != expected.implementation.execution_profile_id
        || gear.implementation_id != expected.implementation.implementation_id
        || gear.artifact_id != expected.implementation.artifact_id
        || gear.inputs != expected.inputs
        || gear.outputs != expected.outputs
        || gear.host_calls != expected.host_calls
        || gear.limits != expected.limits
        || gear.semantic_contract != expected.semantic_contract
        || gear.base.is_some()
        || !gear.resources.is_empty()
        || !gear.authority.is_empty()
    {
        return Err(Refusal::WrongBinding);
    }
    Ok(program)
}

mod fragment;
pub use fragment::PreparedExpressionFragment;

mod factory;
pub use factory::ExpressionOperationFactory;

#[cfg(test)]
mod tests;
