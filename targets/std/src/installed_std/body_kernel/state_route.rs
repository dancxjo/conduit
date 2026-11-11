//! Play-lived typed State in the same shared Body scheduler.
//! State remains owned by its installed Back; no fragment Play or continuation
//! custody is invented for a cell inside the parent Body Play.
use super::*;
use conduit_core::{PlanFragment, PlannedGear};
use conduit_plan_lowering::lowering::LoweredPlanFragment;

pub(super) fn budget(
    placement: &PlannedGear,
    partition: &LoweredPlanFragment,
) -> Result<Option<super::super::back::BackBudget>, String> {
    let Some(state) = partition
        .states
        .iter()
        .find(|state| state.contract.gear_id == placement.gear_id)
    else {
        return Ok(None);
    };
    let maximum = state.contract.maximum_value_bytes;
    Ok(Some(super::super::back::BackBudget {
        value_items: 2,
        value_bytes: maximum
            .checked_mul(2)
            .ok_or("Body State byte budget overflow")?,
        maximum_value_bytes: maximum,
        host_requests: 0,
        sign_items: 16,
    }))
}

pub(super) fn prepare(
    fragment: &PlanFragment,
    partition: &LoweredPlanFragment,
    node: &conduit_plan_lowering::lowering::LoweredNode,
) -> Result<Option<InstalledBack>, String> {
    let Some(state) = partition
        .states
        .iter()
        .find(|state| state.node == node.node)
    else {
        return Ok(None);
    };
    let placement = fragment
        .placements
        .iter()
        .find(|placement| placement.placement_id == node.placement_id)
        .ok_or("Body State has no exact placement")?;
    if state.contract.gear_id != placement.gear_id
        || !fragment
            .states
            .iter()
            .any(|original| original == &state.contract)
    {
        return Err("Body State differs from its sealed fragment contract".into());
    }
    let back = match placement.implementation_id.as_str() {
        conduit_std_offers::STATE_VALUE_STD_IMPLEMENTATION => {
            InstalledBack::TypedState(Box::new(crate::state_value::TypedStateBack::prepare(
                placement,
                &state.contract,
                state.slot,
                state.next,
                state.current,
            )?))
        }
        conduit_std_offers::STATE_VALUE_LARGE_STD_IMPLEMENTATION => InstalledBack::LargeTypedState(
            Box::new(crate::state_value::LargeTypedStateBack::prepare(
                placement,
                &state.contract,
                state.slot,
                state.next,
                state.current,
            )?),
        ),
        _ => return Err("Body State requires an installed Play-lived typed implementation".into()),
    };
    Ok(Some(back))
}
