//! Exact std child Back for the authored Todo scan.

use conduit_composite::{KernelOperationBudget, KernelOperationFactory};
use conduit_core::{
    ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId,
    ImplementationId, PlannedGear,
};
use conduit_kernel::{scheduler::StepBack, HostedValueStore};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
use conduit_todo_plot::{
    todo_combine_kind, TodoCombineBack, COMMAND_MAX_BYTES, STATE_MAX_BYTES, TODO_COMBINE_KIND,
    TODO_COMBINE_REVISION,
};

pub const EXECUTION_PROFILE: &str = "conduit.std/todo-combine-kernel@1";
pub const IMPLEMENTATION: &str = "std/kernel-todo-combine@1";
pub const ARTIFACT: &str = "conduit-std-host/todo-combine@1";

pub struct TodoCombineFactory;

pub fn offer() -> CapabilityOffer {
    BackOfferBuilder::new(
        todo_combine_kind(),
        Back {
            capability_id: CapabilityId::from("std/todo-combine@1"),
            execution_profile_id: ExecutionProfileId::from(EXECUTION_PROFILE),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from(ARTIFACT),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build()
}

fn validate(placement: &PlannedGear) -> Result<(), String> {
    let expected = offer();
    if placement.kind_id.as_str() != TODO_COMBINE_KIND
        || placement.kind_contract_revision.as_str() != TODO_COMBINE_REVISION
        || placement.capability_id != expected.capability_id
        || placement.execution_profile_id != expected.implementation.execution_profile_id
        || placement.implementation_id != expected.implementation.implementation_id
        || placement.artifact_id != expected.implementation.artifact_id
        || placement.inputs != expected.inputs
        || placement.outputs != expected.outputs
        || placement.semantic_contract != expected.semantic_contract
        || placement.limits != expected.limits
        || !placement.configuration.is_empty()
        || placement.base.is_some()
        || !placement.realization_characteristics.is_empty()
        || !placement.realization_properties.is_empty()
        || !placement.terminal_transductions.is_empty()
        || !placement.host_calls.is_empty()
        || !placement.resources.is_empty()
        || !placement.authority.is_empty()
        || !placement.pool_references.is_empty()
    {
        return Err("planned Todo combine Back differs from the exact std offer".into());
    }
    Ok(())
}

impl KernelOperationFactory for TodoCombineFactory {
    fn implementation_id(&self) -> &ImplementationId {
        static ID: std::sync::OnceLock<ImplementationId> = std::sync::OnceLock::new();
        ID.get_or_init(|| ImplementationId::from(IMPLEMENTATION))
    }

    fn budget(&self, placement: &PlannedGear) -> Result<KernelOperationBudget, String> {
        validate(placement)?;
        Ok(KernelOperationBudget {
            value_items: 3,
            value_bytes: (2 * STATE_MAX_BYTES + COMMAND_MAX_BYTES) as u32,
            maximum_value_bytes: STATE_MAX_BYTES as u32,
            host_requests: 0,
            sign_items: 3,
        })
    }

    fn prepare(
        &self,
        placement: &PlannedGear,
        _: &mut HostedValueStore,
    ) -> Result<Box<dyn StepBack<{ FIXED_KERNEL_STORAGE_PORTS_PER_NODE }> + Send>, String> {
        validate(placement)?;
        Ok(Box::new(TodoCombineBack::new()))
    }
}
