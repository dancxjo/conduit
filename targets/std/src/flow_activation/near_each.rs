//! Source-bound pure numerical Each through the ordinary prepared activation.
use super::{install_reusable_planned_activation, NearQuantityFactory, StdActivationHost};
use conduit_composite::{KernelEachBack, KernelOperationFactory};
use conduit_core::*;

pub const MAXIMUM_ITEMS: u16 = 64;
const PROFILE: &str = "conduit.std/near-each-kernel@1";
const IMPLEMENTATION: &str = "std/kernel-near-each@1";
const ARTIFACT: &str = "conduit-composite/kernel-each@1";

pub fn near_each_offer(
    input: &CheckedValueContract,
    output: &CheckedValueContract,
    maximum_items: u16,
) -> Result<CapabilityOffer, String> {
    if maximum_items == 0
        || maximum_items > MAXIMUM_ITEMS
        || input.value_kind.as_str() != QUANTITY_INFO_ID
        || output.value_kind.as_str() != conduit_semantic_catalog::NEAR_QUANTITY_INFO_ID
    {
        return Err("Near Each requires finite Quantity-to-NearQuantity Values".into());
    }
    let kind =
        conduit_semantic_catalog::flow_each_semantic_contract(input, output, None, maximum_items)
            .map_err(str::to_string)?;
    let identity = compute_checked_front_fingerprint(&kind.checked_front());
    Ok(BackOfferBuilder::new(
        kind,
        Back {
            capability_id: format!("std/near-each/{identity}/{maximum_items}").into(),
            execution_profile_id: PROFILE.into(),
            implementation_id: IMPLEMENTATION.into(),
            artifact_id: ARTIFACT.into(),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build())
}

pub(crate) fn validate_near_each(plan: &Plan, planned: &PlannedActivation) -> Result<(), String> {
    if !verify_plan(plan)
        || !verify_plan(&planned.selected_plan)
        || planned.selected_plan_id != planned.selected_plan.plan_id
        || !planned.selected_plan.activations.is_empty()
        || planned.selected_plan.fragments.len() != 1
    {
        return Err("Near Each requires exact flat Source and child Plans".into());
    }
    let owner = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .find(|gear| gear.placement_id == planned.owner_placement_id)
        .ok_or("Near Each owner is absent")?;
    let child = &planned.selected_plan.fragments[0];
    if child.host_id != owner.host_id
        || child.boot_id != owner.boot_id
        || child.offer_generation != owner.offer_generation
        || child.placements.len() != 1
        || !child.connections.is_empty()
        || !child.states.is_empty()
        || !child.shared_pools.is_empty()
        || !child.execution_regions.is_empty()
        || !child.execution_fusions.is_empty()
        || child.fore_ports.len() != 2
    {
        return Err("Near Each requires one exact local pure numerical Gear".into());
    }
    NearQuantityFactory.budget(&child.placements[0])?;
    let contract = |id: &conduit_core::PortId, direction| {
        child
            .fore_ports
            .iter()
            .find(|port| {
                &port.front_port_id == id
                    && port.direction == direction
                    && port.temporal == PortTemporal::Value
                    && port.abnormal_kind.is_none()
                    && port.selected_line.is_none()
                    && port.track == ConnectionTrack::Payload
                    && port.item_capacity == 1
            })
            .and_then(|port| port.value_contract.as_ref())
            .ok_or("Near Each requires exact finite child Value Fores")
    };
    let expected = near_each_offer(
        contract(&planned.input.front_port_id, PortDirection::Input)?,
        contract(&planned.output.front_port_id, PortDirection::Output)?,
        planned.limits.maximum_items,
    )?;
    if owner.kind_id != expected.kind_id
        || owner.kind_contract_revision != expected.kind_contract_revision
        || owner.capability_id != expected.capability_id
        || owner.implementation_id != expected.implementation.implementation_id
        || owner.execution_profile_id != expected.implementation.execution_profile_id
        || owner.artifact_id != expected.implementation.artifact_id
        || owner.inputs != expected.inputs
        || owner.outputs != expected.outputs
        || owner.semantic_contract != expected.semantic_contract
        || owner.limits != expected.limits
        || !owner.configuration.is_empty()
        || owner.base.is_some()
        || !owner.host_calls.is_empty()
        || !owner.resources.is_empty()
        || !owner.authority.is_empty()
        || !owner.pool_references.is_empty()
        || !owner.realization_properties.is_empty()
        || !owner.realization_characteristics.is_empty()
        || !owner.terminal_transductions.is_empty()
    {
        return Err("Near Each parent differs from its exact installed offer".into());
    }
    Ok(())
}

pub fn install_pure_near_each(
    plan: &Plan,
    prepared: &mut PreparedPlan,
    activation_id: &str,
    host: &mut StdActivationHost,
) -> Result<KernelEachBack, String> {
    let planned = plan
        .activations
        .iter()
        .find_map(|entry| match entry {
            PlannedActivationEntry::Unary(value) if value.activation_id == activation_id => {
                Some(value)
            }
            _ => None,
        })
        .ok_or("Near Each activation is absent")?;
    validate_near_each(plan, planned)?;
    let each = install_reusable_planned_activation(plan, prepared, activation_id, host)
        .map_err(|e| format!("Near Each child receipt handoff: {e:?}"))?
        .into_unary()
        .map_err(|e| format!("Near Each coordinator: {e:?}"))?;
    let steps = u32::from(planned.limits.maximum_items)
        .checked_mul(2)
        .ok_or("Near Each work overflow")?;
    KernelEachBack::prepare(each, planned, MAXIMUM_ITEMS, steps)
}
