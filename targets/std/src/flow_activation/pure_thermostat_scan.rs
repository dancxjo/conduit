//! Production child-pool handoff for the pure Thermostat scan Plan.

use super::{install_planned_activation, StdActivationHost, ThermostatCombineFactory};
use conduit_composite::{BoundedScanActivationHost, KernelOperationFactory};
use conduit_core::{verify_plan, Plan, PlannedActivationEntry, PlannedGear, PreparedPlan};
use conduit_thermostat_plot::THERMOSTAT_KIND;

pub fn install_pure_thermostat_scan(
    plan: &Plan,
    prepared: &mut PreparedPlan,
    activation_id: &str,
    host: &mut StdActivationHost,
) -> Result<BoundedScanActivationHost, String> {
    let child = exact_pure_child(plan, activation_id)?;
    ThermostatCombineFactory.budget(child)?;
    install_planned_activation(plan, prepared, activation_id, host)
        .map_err(|error| format!("prepare exact Thermostat scan: {error:?}"))?
        .into_scan()
        .map_err(|error| format!("prepare exact Thermostat scan: {error:?}"))
}

pub(super) fn exact_pure_child<'a>(
    plan: &'a Plan,
    activation_id: &str,
) -> Result<&'a PlannedGear, String> {
    if !verify_plan(plan) {
        return Err("Thermostat scan requires a verified whole Plan".into());
    }
    let mut matches = plan.activations.iter().filter(|entry| match entry {
        PlannedActivationEntry::Unary(value) => value.activation_id == activation_id,
        PlannedActivationEntry::Fold(value) => value.activation_id == activation_id,
        PlannedActivationEntry::Scan(value) => value.activation_id == activation_id,
    });
    let entry = matches
        .next()
        .ok_or("Thermostat scan has no exact planned activation")?;
    if matches.next().is_some() {
        return Err("Thermostat scan activation identity is ambiguous".into());
    }
    let PlannedActivationEntry::Scan(scan) = entry else {
        return Err("Thermostat scan activation has another coordinator Kind".into());
    };
    let owner = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .find(|placement| placement.placement_id == scan.owner_placement_id)
        .ok_or("Thermostat scan has no planned parent placement")?;
    if owner.kind_id.as_str() != conduit_semantic_catalog::FLOW_SCAN_KIND
        || scan.selected_plan_id != scan.selected_plan.plan_id
        || !verify_plan(&scan.selected_plan)
        || !scan.selected_plan.activations.is_empty()
        || scan.selected_plan.fragments.len() != 1
    {
        return Err("Thermostat scan requires one exact, non-nested child Plan".into());
    }
    let fragment = &scan.selected_plan.fragments[0];
    if fragment.host_id != owner.host_id
        || fragment.boot_id != owner.boot_id
        || fragment.offer_generation != owner.offer_generation
        || fragment.placements.len() != 1
        || !fragment.states.is_empty()
    {
        return Err("Thermostat scan child belongs to another Host or is not one pure Gear".into());
    }
    let child = &fragment.placements[0];
    if child.kind_id.as_str() != THERMOSTAT_KIND
        || !child.host_calls.is_empty()
        || !child.resources.is_empty()
        || !child.authority.is_empty()
        || child.base.is_some()
    {
        return Err(
            "Thermostat scan child effects or Kind are not supported by the pure std route".into(),
        );
    }
    Ok(child)
}
