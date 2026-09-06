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
        let factory = factory(&placement.implementation_id)
            .ok_or_else(|| "planned implementation is not installed".to_string())?;
        operations.push((factory.prepare)(placement, values)?);
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
