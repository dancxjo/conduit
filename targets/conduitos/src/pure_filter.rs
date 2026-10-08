//! Target-owned realization for checked Source predicates and original-frame forwarding.
use alloc::{boxed::Box, format, string::String, vec, vec::Vec};
pub use common::PreparedPureFilter;
use conduit_composite::{KernelOperationBudget, KernelOperationFactory};
use conduit_core::*;
use conduit_kernel::{HostedValueStore, scheduler::StepBack};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
use conduit_semantic_catalog::operation_owners::pure_filter as common;

pub const IMPLEMENTATION: &str = "conduitos/kernel-pure-filter@1";
pub const PROFILE: &str = "conduitos/pure-filter-cooperative-bounded@1";
pub const ARTIFACT: &str = "conduit-plot/pure-filter@1";
pub const CALL: &str = "conduit.host/pure-filter@1";
pub fn offer(
    program: &conduit_plot::PortableExpressionProgram,
    temporal: PortTemporal,
) -> Result<CapabilityOffer, StructuredInfoRefusal> {
    let contract = conduit_semantic_catalog::pure_filter_contract(program, temporal)?;
    let target_kind = contract.kind_id.clone();
    Ok(BackOfferBuilder::new(
        contract,
        Back {
            capability_id: format!("conduitos/{}", target_kind.as_str()).into(),
            execution_profile_id: PROFILE.into(),
            implementation_id: IMPLEMENTATION.into(),
            artifact_id: ARTIFACT.into(),
            host_calls: vec![HostCallRequirement {
                contract_id: CALL.into(),
                target_kind: Some(target_kind),
                maximum_in_flight: 1,
                maximum_input_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
                maximum_output_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
            }],
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build())
}
pub struct PureFilterOperationFactory(common::PureFilterOperationFactory);
impl PureFilterOperationFactory {
    pub(super) fn plan_id(&self) -> &PlanId {
        self.0.plan_id()
    }
    pub fn for_plan(plan: &Plan) -> Result<Self, String> {
        common::PureFilterOperationFactory::for_plan(plan, IMPLEMENTATION.into(), offer).map(Self)
    }
    pub fn prepare_host(&self, gear: &PlannedGear) -> Result<PreparedPureFilter, String> {
        self.0.prepare_host(gear)
    }
}
impl KernelOperationFactory for PureFilterOperationFactory {
    fn implementation_id(&self) -> &ImplementationId {
        self.0.implementation_id()
    }
    fn budget(&self, gear: &PlannedGear) -> Result<KernelOperationBudget, String> {
        self.0.budget(gear)
    }
    fn prepare(
        &self,
        gear: &PlannedGear,
        values: &mut HostedValueStore,
    ) -> Result<Box<dyn StepBack<FIXED_KERNEL_STORAGE_PORTS_PER_NODE> + Send>, String> {
        self.0.prepare(gear, values)
    }
}
mod call;
pub use call::{FilterCallRefusal, FilterHostCall};

#[cfg(test)]
mod tests;
