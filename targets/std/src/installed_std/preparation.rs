//! Construct the finite operation driver set before Play start.
use super::{catalog::factory, operation::InstalledOperation, MAX_NODES, PORTS};
use conduit_core::PlanFragment;
use conduit_kernel::{scheduler::OperationDriver, HostedValueStore};
use conduit_plan_lowering::lowering::LoweredPlanFragment;

pub(super) fn prepare_operations(
    fragment: &PlanFragment,
    lowered: &LoweredPlanFragment,
    values: &mut HostedValueStore,
    play: &conduit_core::ActivePlayIdentity,
    mut retained: Option<&mut Vec<crate::state_value::RetainedTypedState>>,
) -> Result<[OperationDriver<InstalledOperation, PORTS>; MAX_NODES], String> {
    if lowered.identity.plan_id != fragment.plan_id
        || lowered.identity.fragment_id != fragment.fragment_id
        || play.plan_id != fragment.plan_id
        || play.host_id != fragment.host_id
        || play.boot_id != fragment.boot_id
    {
        return Err("operation preparation requires the exact partition Plan and Play".into());
    }
    let mut occupied = [false; MAX_NODES];
    for node in &lowered.nodes {
        let slot = occupied
            .get_mut(usize::from(node.node.0))
            .ok_or_else(|| "lowered node exceeds installed driver capacity".to_string())?;
        if *slot {
            return Err("duplicate lowered operation node".into());
        }
        *slot = true;
    }
    validate_retained_inputs(
        fragment,
        lowered,
        play,
        retained.as_deref().map(Vec::as_slice).unwrap_or(&[]),
    )?;
    // Kernel IDs may be offset within a combined workload. Resolve authored
    // placement identity inside this exact partition, never by the global ID.
    let mut operations: Vec<_> = (0..MAX_NODES)
        .map(|_| InstalledOperation::inactive())
        .collect();
    for node in &lowered.nodes {
        let placement = fragment
            .placements
            .iter()
            .find(|placement| placement.placement_id == node.placement_id)
            .ok_or_else(|| "lowered node has no planned placement".to_string())?;
        if let Some(state) = lowered.states.iter().find(|state| state.node == node.node) {
            if state.contract.retained.is_some() {
                continue;
            }
            operations[usize::from(node.node.0)] = InstalledOperation::TypedState(Box::new(
                crate::state_value::TypedStateOperation::prepare_for_play(fragment, state, play)?,
            ));
        } else {
            let factory = factory(&placement.implementation_id).ok_or_else(|| {
                "planned implementation is not installed or lacks sealed State".to_string()
            })?;
            operations[usize::from(node.node.0)] = (factory.prepare)(placement, values)?;
        }
    }
    // Ordinary/fresh preparation finishes before any incoming cell is consumed.
    for state in lowered
        .states
        .iter()
        .filter(|state| state.contract.retained.is_some())
    {
        let sources = retained
            .as_deref_mut()
            .expect("retained inputs were validated");
        let index = sources
            .iter()
            .position(|source| source.provenance().source_state == state.contract.state_id)
            .expect("every retained obligation has one owned source");
        let source = sources.remove(index);
        let operation = match crate::state_value::TypedStateOperation::prepare_continued(
            fragment, state, play, source,
        ) {
            Ok(operation) => operation,
            Err(failure) => {
                sources.insert(index, failure.source);
                return Err(failure.reason);
            }
        };
        operations[usize::from(state.node.0)] = InstalledOperation::TypedState(Box::new(operation));
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

#[cfg(test)]
#[path = "preparation_tests.rs"]
mod tests;

pub(crate) fn lower_fragment_with_continuity(
    fragment: &PlanFragment,
    continuity: bool,
) -> Result<LoweredPlanFragment, String> {
    let mut profile = state_storage_profile();
    if continuity {
        profile = profile.with_owned_state_continuity();
    }
    conduit_plan_lowering::lowering::lower_plan_fragment_for_profile(fragment, profile)
        .map_err(|error| format!("lowering: {error:?}"))
}

pub(crate) fn state_storage_profile() -> conduit_plan_lowering::lowering::KernelStorageProfile {
    conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PROFILE
        .with_state_storage(
            MAX_NODES as u16,
            conduit_std_offers::STATE_VALUE_STD_MAXIMUM_BYTES,
        )
        .expect("the installed State storage profile has fixed positive capacities")
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

pub(crate) fn validate_retained_inputs(
    fragment: &PlanFragment,
    lowered: &LoweredPlanFragment,
    play: &conduit_core::ActivePlayIdentity,
    sources: &[crate::state_value::RetainedTypedState],
) -> Result<(), String> {
    let obligations = lowered
        .states
        .iter()
        .filter(|state| state.contract.retained.is_some())
        .count();
    if sources.len() != obligations || sources.len() > MAX_NODES {
        return Err("owned State count differs from sealed continuity obligations".into());
    }
    for state in lowered
        .states
        .iter()
        .filter(|state| state.contract.retained.is_some())
    {
        let mut matches = sources
            .iter()
            .filter(|source| source.provenance().source_state == state.contract.state_id);
        let source = matches
            .next()
            .ok_or_else(|| "retained State owner is missing".to_string())?;
        if matches.next().is_some() {
            return Err("duplicate retained State owner".into());
        }
        crate::state_value::TypedStateOperation::validate_continuation(
            fragment, state, play, source,
        )?;
    }
    Ok(())
}
