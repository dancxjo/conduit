//! Pre-Play numeric composition of exact local Form partitions.
//! Original Plan/fragment identities remain on each partition. This creates
//! neither an authored Form nor a synthetic Plan and performs no scheduling.
use crate::lowering::{
    lower_plan_fragment_for_profile, KernelStorageProfile, LoweredPlanFragment, LoweringError,
};
use alloc::vec::Vec;
use conduit_core::PlanFragment;
use conduit_kernel::{CordEndpoint, SignExpectationTarget};

#[derive(Clone, Copy, Debug)]
pub struct FragmentSetBounds {
    pub fragments: u16,
    pub nodes: u16,
    pub cords: u16,
    pub queue_slots: u16,
    pub value_bytes: u32,
    pub sign_items: u16,
    pub sign_bytes: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FragmentSetError {
    Empty,
    Capacity,
    DifferentHostBootOrGeneration,
    DuplicateFragment,
    RemoteUnsupported,
    StateUnsupported,
    FusionUnsupported,
    SharedPoolUnsupported,
    Fragment(LoweringError),
}

#[derive(Debug)]
pub struct LoweredFragmentSet {
    /// Numeric IDs are global to this set; string identities remain original.
    pub partitions: Vec<LoweredPlanFragment>,
    pub nodes: u16,
    pub cords: u16,
    pub queue_slots: u16,
    pub value_bytes: u32,
    pub sign_items: u16,
    pub sign_bytes: u32,
}

/// Lower local partitions into disjoint kernel tables. The caller must validate
/// the owning BodyPlan/workset and reserve combined resources before Play.
/// Retained State, fusion, shared pools and remote endpoints need their own
/// cross-partition contracts; they refuse rather than being silently discarded.
pub fn lower_local_fragment_set(
    fragments: &[&PlanFragment],
    profile: KernelStorageProfile,
    bounds: FragmentSetBounds,
) -> Result<LoweredFragmentSet, FragmentSetError> {
    let first = fragments.first().ok_or(FragmentSetError::Empty)?;
    if fragments.len() > usize::from(bounds.fragments) {
        return Err(FragmentSetError::Capacity);
    }
    let mut result = LoweredFragmentSet {
        partitions: Vec::with_capacity(fragments.len()),
        nodes: 0,
        cords: 0,
        queue_slots: 0,
        value_bytes: 0,
        sign_items: 0,
        sign_bytes: 0,
    };
    let mut route_targets = 0_u16;
    let mut sign_expectations = 0_u16;
    for (index, fragment) in fragments.iter().enumerate() {
        if fragment.host_id != first.host_id
            || fragment.boot_id != first.boot_id
            || fragment.offer_generation != first.offer_generation
        {
            return Err(FragmentSetError::DifferentHostBootOrGeneration);
        }
        if fragments[..index].iter().any(|prior| {
            prior.plan_id == fragment.plan_id && prior.fragment_id == fragment.fragment_id
        }) {
            return Err(FragmentSetError::DuplicateFragment);
        }
        let mut lowered = lower_plan_fragment_for_profile(fragment, profile)
            .map_err(FragmentSetError::Fragment)?;
        if !lowered.remote_endpoints.is_empty() {
            return Err(FragmentSetError::RemoteUnsupported);
        }
        if !lowered.states.is_empty() {
            return Err(FragmentSetError::StateUnsupported);
        }
        if !lowered.fusions.is_empty() {
            return Err(FragmentSetError::FusionUnsupported);
        }
        if !lowered.shared_pools.is_empty() {
            return Err(FragmentSetError::SharedPoolUnsupported);
        }
        let next_nodes = bounded(result.nodes, count(lowered.nodes.len())?, bounds.nodes)?;
        let next_cords = bounded(result.cords, count(lowered.cords.len())?, bounds.cords)?;
        let next_slots = bounded(
            result.queue_slots,
            lowered.cord_value_slots,
            bounds.queue_slots,
        )?;
        let next_values = result
            .value_bytes
            .checked_add(lowered.cord_value_bytes)
            .filter(|value| *value <= bounds.value_bytes)
            .ok_or(FragmentSetError::Capacity)?;
        let next_sign_items = bounded(result.sign_items, lowered.sign_items, bounds.sign_items)?;
        let next_sign_bytes = result
            .sign_bytes
            .checked_add(lowered.sign_bytes)
            .filter(|value| *value <= bounds.sign_bytes)
            .ok_or(FragmentSetError::Capacity)?;
        let targets = lowered.routes.iter().try_fold(0_u16, |total, route| {
            add(total, count(route.targets.len())?)
        })?;
        let next_targets = add(route_targets, targets)?;
        let next_expectations = add(sign_expectations, count(lowered.signs.len())?)?;
        reindex(
            &mut lowered,
            result.nodes,
            result.cords,
            result.queue_slots,
            route_targets,
            sign_expectations,
        )?;
        result.partitions.push(lowered);
        result.nodes = next_nodes;
        result.cords = next_cords;
        result.queue_slots = next_slots;
        result.value_bytes = next_values;
        result.sign_items = next_sign_items;
        result.sign_bytes = next_sign_bytes;
        route_targets = next_targets;
        sign_expectations = next_expectations;
    }
    Ok(result)
}

fn count(value: usize) -> Result<u16, FragmentSetError> {
    value.try_into().map_err(|_| FragmentSetError::Capacity)
}
fn add(left: u16, right: u16) -> Result<u16, FragmentSetError> {
    left.checked_add(right).ok_or(FragmentSetError::Capacity)
}
fn bounded(left: u16, right: u16, maximum: u16) -> Result<u16, FragmentSetError> {
    let value = add(left, right)?;
    if value > maximum {
        Err(FragmentSetError::Capacity)
    } else {
        Ok(value)
    }
}
fn endpoint(endpoint: &mut CordEndpoint, nodes: u16) -> Result<(), FragmentSetError> {
    match endpoint {
        CordEndpoint::Local { node, .. } => node.0 = add(node.0, nodes)?,
        CordEndpoint::Remote(_) => return Err(FragmentSetError::RemoteUnsupported),
    }
    Ok(())
}

fn reindex(
    part: &mut LoweredPlanFragment,
    nodes: u16,
    cords: u16,
    slots: u16,
    targets: u16,
    signs: u16,
) -> Result<(), FragmentSetError> {
    for node in &mut part.nodes {
        node.node.0 = add(node.node.0, nodes)?;
        for port in node.inputs.iter_mut().chain(&mut node.outputs) {
            port.node.0 = add(port.node.0, nodes)?;
        }
    }
    for spec in &mut part.node_specs {
        for cord in spec.input_cords.iter_mut().flatten() {
            cord.0 = add(cord.0, cords)?;
        }
    }
    for cord in &mut part.cords {
        cord.spec.cord.0 = add(cord.spec.cord.0, cords)?;
        cord.spec.slot_start = add(cord.spec.slot_start, slots)?;
        endpoint(&mut cord.spec.source, nodes)?;
        endpoint(&mut cord.spec.sink, nodes)?;
    }
    for route in &mut part.routes {
        route.source_node.0 = add(route.source_node.0, nodes)?;
        route.range.start = add(route.range.start, targets)?;
        for target in &mut route.targets {
            target.cord.0 = add(target.cord.0, cords)?;
            endpoint(&mut target.sink, nodes)?;
        }
    }
    for operation in &mut part.host_operations {
        operation.node.0 = add(operation.node.0, nodes)?;
    }
    for resource in &mut part.resources {
        resource.node.0 = add(resource.node.0, nodes)?;
    }
    for sign in &mut part.signs {
        sign.expectation.0 = add(sign.expectation.0, signs)?;
        match &mut sign.target {
            SignExpectationTarget::Node(node) => node.0 = add(node.0, nodes)?,
            SignExpectationTarget::Cord(cord) => cord.0 = add(cord.0, cords)?,
            SignExpectationTarget::Fragment => {}
        }
    }
    for (node, _) in &mut part.identity.placements {
        node.0 = add(node.0, nodes)?;
    }
    for port in &mut part.identity.ports {
        port.node.0 = add(port.node.0, nodes)?;
    }
    for (cord, _) in &mut part.identity.connections {
        cord.0 = add(cord.0, cords)?;
    }
    for (node, _, _) in &mut part.identity.host_operations {
        node.0 = add(node.0, nodes)?;
    }
    for (node, _, _) in &mut part.identity.resources {
        node.0 = add(node.0, nodes)?;
    }
    Ok(())
}
