//! No-allocation admission of common local lowering's full owned highwater.
use super::*;
use crate::lowering::fragment_lowering_layout;
use core::mem::size_of;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FragmentSetLayout {
    pub peak_owned_bytes: usize,
    pub retained_owned_bytes: usize,
    pub nodes: u16,
    pub cords: u16,
    pub queue_slots: u16,
    pub value_bytes: u32,
    pub sign_items: u16,
    pub sign_bytes: u32,
    pub fore_ports: u16,
}

/// Borrowed shape only; the funded materializer still verifies every complete
/// sealed Plan through the ordinary Core verifier. A descriptor is no grant.
pub fn lower_local_fragment_set_layout_from_plans(
    plans: &[&Plan],
    profile: KernelStorageProfile,
    bounds: FragmentSetBounds,
) -> Result<FragmentSetLayout, FragmentSetError> {
    let first = plans
        .first()
        .and_then(|p| p.fragments.first())
        .ok_or(FragmentSetError::Empty)?;
    if plans.len() > usize::from(bounds.fragments) {
        return Err(capacity(
            "fragments",
            plans.len() as u64,
            bounds.fragments.into(),
        ));
    }
    let table = plans
        .len()
        .checked_mul(size_of::<LoweredPlanFragment>())
        .filter(|n| *n <= isize::MAX as usize)
        .ok_or(FragmentSetError::Fragment(LoweringError::CapacityOverflow))?;
    let mut layout = FragmentSetLayout {
        peak_owned_bytes: 0,
        retained_owned_bytes: table,
        nodes: 0,
        cords: 0,
        queue_slots: 0,
        value_bytes: 0,
        sign_items: 0,
        sign_bytes: 0,
        fore_ports: 0,
    };
    let mut temporary = 0;
    let mut targets = 0u16;
    let mut expectations = 0u16;
    for (index, plan) in plans.iter().enumerate() {
        let fragment = plan
            .fragments
            .first()
            .filter(|_| plan.fragments.len() == 1)
            .ok_or(FragmentSetError::Fragment(LoweringError::InvalidFragment))?;
        if fragment.host_id != first.host_id
            || fragment.boot_id != first.boot_id
            || fragment.offer_generation != first.offer_generation
        {
            return Err(FragmentSetError::DifferentHostBootOrGeneration);
        }
        if plans[..index].iter().any(|p| {
            p.fragments
                .iter()
                .any(|f| f.plan_id == fragment.plan_id && f.fragment_id == fragment.fragment_id)
        }) {
            return Err(FragmentSetError::DuplicateFragment);
        }
        let verifier = conduit_core::local_plan_verification_backing_bytes(plan).ok_or(
            FragmentSetError::Fragment(LoweringError::UnsupportedOwnedLayout),
        )?;
        let part = fragment_lowering_layout(fragment).map_err(FragmentSetError::Fragment)?;
        for placement in &fragment.placements {
            if placement.inputs.len() > profile.maximum_ports_per_node()
                || placement.outputs.len() > profile.maximum_ports_per_node()
            {
                return Err(FragmentSetError::Fragment(
                    LoweringError::UnsupportedOwnedLayout,
                ));
            }
        }
        layout.retained_owned_bytes = layout
            .retained_owned_bytes
            .checked_add(part.retained_owned_bytes)
            .ok_or(FragmentSetError::Fragment(LoweringError::CapacityOverflow))?;
        temporary = temporary.max(verifier).max(part.refusal_owned_bytes);
        layout.nodes = bounded(
            "nodes",
            layout.nodes,
            count("nodes", part.nodes)?,
            bounds.nodes,
        )?;
        layout.cords = bounded(
            "cords",
            layout.cords,
            count("cords", part.cords)?,
            bounds.cords,
        )?;
        layout.queue_slots = bounded(
            "queue-slots",
            layout.queue_slots,
            part.queue_slots,
            bounds.queue_slots,
        )?;
        layout.value_bytes = layout
            .value_bytes
            .checked_add(part.value_bytes)
            .filter(|n| *n <= bounds.value_bytes)
            .ok_or_else(|| {
                capacity(
                    "value-bytes",
                    u64::from(layout.value_bytes) + u64::from(part.value_bytes),
                    bounds.value_bytes.into(),
                )
            })?;
        layout.sign_items = bounded(
            "sign-items",
            layout.sign_items,
            fragment.sign_storage_budget.item_capacity,
            bounds.sign_items,
        )?;
        layout.sign_bytes = layout
            .sign_bytes
            .checked_add(fragment.sign_storage_budget.byte_capacity)
            .filter(|n| *n <= bounds.sign_bytes)
            .ok_or_else(|| {
                capacity(
                    "sign-bytes",
                    u64::from(layout.sign_bytes)
                        + u64::from(fragment.sign_storage_budget.byte_capacity),
                    bounds.sign_bytes.into(),
                )
            })?;
        layout.fore_ports = add(
            "fore-endpoints",
            layout.fore_ports,
            count("fore-endpoints", part.fore_ports)?,
        )?;
        targets = add(
            "route-targets",
            targets,
            count("route-targets", part.route_targets)?,
        )?;
        expectations = add(
            "sign-expectations",
            expectations,
            count("sign-expectations", fragment.expected_sign.len())?,
        )?;
    }
    // Every final partition and all rich ID clones coexist. Add verifier or
    // refusal scratch on top, rather than forgetting earlier retained parts.
    layout.peak_owned_bytes = layout
        .retained_owned_bytes
        .checked_add(temporary)
        .ok_or(FragmentSetError::Fragment(LoweringError::CapacityOverflow))?;
    Ok(layout)
}

/// Refuse a shortage before verifier scratch, tables, Strings, or staging owners
/// are allocated. The caller must reserve this same original Host counter first.
pub fn lower_local_fragment_set_from_plans_with_limit(
    plans: &[&Plan],
    profile: KernelStorageProfile,
    bounds: FragmentSetBounds,
    maximum_owned_bytes: usize,
) -> Result<LoweredFragmentSet, FragmentSetError> {
    let layout = lower_local_fragment_set_layout_from_plans(plans, profile, bounds)?;
    if layout.peak_owned_bytes > maximum_owned_bytes {
        return Err(capacity(
            "lowering-owned-bytes",
            layout.peak_owned_bytes as u64,
            maximum_owned_bytes as u64,
        ));
    }
    let lowered = lower_local_fragment_set_from_plans(plans, profile, bounds)?;
    let retained = lowered_local_fragment_set_backing_bytes(&lowered).ok_or(
        FragmentSetError::Fragment(LoweringError::UnsupportedOwnedLayout),
    )?;
    if retained > layout.retained_owned_bytes {
        return Err(capacity(
            "lowering-retained-bytes",
            retained as u64,
            layout.retained_owned_bytes as u64,
        ));
    }
    Ok(lowered)
}
