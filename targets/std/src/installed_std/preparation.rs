//! Construct the finite operation driver set before Play start.
use super::{catalog::factory, operation::InstalledOperation, MAX_NODES, PORTS};
use conduit_core::PlanFragment;
use conduit_kernel::{scheduler::OperationDriver, HostedValueStore};
use conduit_plan_lowering::lowering::LoweredPlanFragment;

pub(super) fn prepare_operations(
    fragment: &PlanFragment,
    lowered: &LoweredPlanFragment,
    values: &mut HostedValueStore,
) -> Result<[OperationDriver<InstalledOperation, PORTS>; MAX_NODES], String> {
    let mut operations = Vec::with_capacity(MAX_NODES);
    for node in &lowered.nodes {
        let placement = fragment
            .placements
            .get(usize::from(node.node.0))
            .ok_or_else(|| "lowered node has no planned placement".to_string())?;
        if let Some(state) = lowered.states.iter().find(|state| state.node == node.node) {
            operations.push(InstalledOperation::TypedState(Box::new(
                crate::state_value::TypedStateOperation::prepare(
                    placement,
                    &state.contract,
                    state.slot,
                    state.next,
                    state.current,
                )?,
            )));
        } else {
            let factory = factory(&placement.implementation_id).ok_or_else(|| {
                "planned implementation is not installed or lacks sealed State".to_string()
            })?;
            operations.push((factory.prepare)(placement, values)?);
        }
    }
    while operations.len() < MAX_NODES {
        operations.push(InstalledOperation::inactive());
    }
    let drivers: [OperationDriver<InstalledOperation, PORTS>; MAX_NODES] = operations
        .into_iter()
        .map(|operation| {
            OperationDriver::new(operation)
                .map_err(|error| format!("prepare installed operation: {error:?}"))
        })
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| "installed driver capacity changed".to_string())?;
    Ok(drivers)
}

pub(super) fn lower_fragment(fragment: &PlanFragment) -> Result<LoweredPlanFragment, String> {
    let profile = conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PROFILE
        .with_state_storage(
            MAX_NODES as u16,
            conduit_std_offers::STATE_VALUE_STD_MAXIMUM_BYTES,
        )
        .map_err(|error| format!("State storage profile: {error:?}"))?;
    conduit_plan_lowering::lowering::lower_plan_fragment_for_profile(fragment, profile)
        .map_err(|error| format!("lowering: {error:?}"))
}

pub(super) fn operation_budget(
    placement: &conduit_core::PlannedGear,
) -> Result<super::factory::OperationBudget, String> {
    if placement.implementation_id.as_str() == conduit_std_offers::STATE_VALUE_STD_IMPLEMENTATION {
        // Reserve the selected implementation's fixed envelope. Exact authored
        // initialization and the sealed per-cell capacity are checked on prepare.
        Ok(super::factory::OperationBudget {
            value_items: 2,
            value_bytes: 128,
            host_requests: 0,
            sign_items: 16,
            maximum_value_bytes: 64,
        })
    } else {
        let factory = factory(&placement.implementation_id)
            .ok_or_else(|| "planned implementation is not installed".to_string())?;
        (factory.budget)(placement)
    }
}
