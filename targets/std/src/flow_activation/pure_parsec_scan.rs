//! Receipt-backed installation of one exact source-evaluated Parsec scan.
use super::{install_planned_activation, parsec_combine, StdActivationHost};
use conduit_composite::BoundedScanActivationHost;
use conduit_core::{verify_plan, Plan, PlannedActivationEntry, PlannedGear, PreparedPlan};

pub fn install_pure_parsec_scan(
    plan: &Plan,
    prepared: &mut PreparedPlan,
    activation_id: &str,
    host: &mut StdActivationHost,
) -> Result<BoundedScanActivationHost, String> {
    exact_pure_child(plan, activation_id)?;
    install_planned_activation(plan, prepared, activation_id, host)
        .map_err(|e| format!("prepare exact Parsec scan: {e:?}"))?
        .into_scan()
        .map_err(|e| format!("prepare exact Parsec scan: {e:?}"))
}
pub(super) fn exact_pure_child<'a>(
    plan: &'a Plan,
    activation_id: &str,
) -> Result<&'a PlannedGear, String> {
    if !verify_plan(plan) {
        return Err("Parsec scan requires verified whole Plan".into());
    }
    let mut entries = plan.activations.iter().filter(|entry| match entry {
        PlannedActivationEntry::Unary(entry) => entry.activation_id == activation_id,
        PlannedActivationEntry::Fold(entry) => entry.activation_id == activation_id,
        PlannedActivationEntry::Scan(entry) => entry.activation_id == activation_id,
    });
    let entry = entries
        .next()
        .ok_or("Parsec scan has no selected activation")?;
    if entries.next().is_some() {
        return Err("Parsec scan activation is ambiguous".into());
    }
    let PlannedActivationEntry::Scan(scan) = entry else {
        return Err("Parsec selected activation is not scan".into());
    };
    let owner = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .find(|placement| placement.placement_id == scan.owner_placement_id)
        .ok_or("Parsec scan has no selected owner")?;
    super::validate_planned_parsec_scan(owner, scan)?;
    if scan.selected_plan_id != scan.selected_plan.plan_id
        || !verify_plan(&scan.selected_plan)
        || !scan.selected_plan.activations.is_empty()
        || scan.selected_plan.fragments.len() != 1
    {
        return Err("Parsec scan has a different or nested child Plan".into());
    }
    let fragment = &scan.selected_plan.fragments[0];
    if fragment.host_id != owner.host_id
        || fragment.boot_id != owner.boot_id
        || fragment.offer_generation != owner.offer_generation
        || fragment.placements.len() != 1
        || !fragment.states.is_empty()
    {
        return Err("Parsec child has a different Host/Boot or state owner".into());
    }
    let child = &fragment.placements[0];
    parsec_combine::validate_placement(child)?;
    parsec_combine::maximum_scan_child_steps(scan)?;
    Ok(child)
}
