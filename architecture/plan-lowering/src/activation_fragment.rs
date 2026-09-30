//! Whole-Plan-aware lowering of activation coordinators owned by one fragment.
//!
//! This deliberately stops before executable target installation: mapping a
//! selected child Plan onto target Host Calls requires a separately reviewed
//! authority contract.

use alloc::vec::Vec;
use conduit_core::{verify_plan, FragmentId, Plan, PlanId, PlannedActivationEntry};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoweredFragmentActivations {
    pub source_plan_id: PlanId,
    pub fragment_id: FragmentId,
    pub fingerprint: [u8; 32],
    pub entries: Vec<PlannedActivationEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ActivationLoweringError {
    InvalidPlan,
    UnknownFragment(FragmentId),
    StaleFragmentIdentity,
    ForeignOrDuplicateOwner,
}

pub fn lower_fragment_activations(
    plan: &Plan,
    fragment_id: &FragmentId,
) -> Result<LoweredFragmentActivations, ActivationLoweringError> {
    if !verify_plan(plan) {
        return Err(ActivationLoweringError::InvalidPlan);
    }
    let mut fragments = plan
        .fragments
        .iter()
        .filter(|part| &part.fragment_id == fragment_id);
    let fragment = fragments
        .next()
        .ok_or_else(|| ActivationLoweringError::UnknownFragment(fragment_id.clone()))?;
    if fragments.next().is_some() || fragment.plan_id != plan.plan_id {
        return Err(ActivationLoweringError::StaleFragmentIdentity);
    }
    let mut entries = Vec::new();
    for entry in &plan.activations {
        let owner = owner(entry);
        let count = plan
            .fragments
            .iter()
            .flat_map(|part| &part.placements)
            .filter(|placement| &placement.placement_id == owner)
            .count();
        if count != 1 {
            return Err(ActivationLoweringError::ForeignOrDuplicateOwner);
        }
        if fragment
            .placements
            .iter()
            .any(|placement| &placement.placement_id == owner)
        {
            entries.push(entry.clone());
        }
    }
    let fingerprint = fingerprint(&plan.plan_id, fragment_id, &entries);
    Ok(LoweredFragmentActivations {
        source_plan_id: plan.plan_id.clone(),
        fragment_id: fragment_id.clone(),
        fingerprint,
        entries,
    })
}

pub fn verify_lowered_fragment_activations(
    lowered: &LoweredFragmentActivations,
    plan: &Plan,
) -> bool {
    lower_fragment_activations(plan, &lowered.fragment_id).as_ref() == Ok(lowered)
}

fn owner(entry: &PlannedActivationEntry) -> &conduit_core::PlacementId {
    match entry {
        PlannedActivationEntry::Unary(value) => &value.owner_placement_id,
        PlannedActivationEntry::Fold(value) => &value.owner_placement_id,
        PlannedActivationEntry::Scan(value) => &value.owner_placement_id,
    }
}

fn fingerprint(
    plan: &PlanId,
    fragment: &FragmentId,
    entries: &[PlannedActivationEntry],
) -> [u8; 32] {
    let mut canonical = Vec::new();
    canonical.extend_from_slice(plan.as_str().as_bytes());
    canonical.push(0);
    canonical.extend_from_slice(fragment.as_str().as_bytes());
    for entry in entries {
        let (tag, id, selected, limits) = match entry {
            PlannedActivationEntry::Unary(value) => (
                0,
                &value.activation_id,
                &value.selected_plan_id,
                value.limits,
            ),
            PlannedActivationEntry::Fold(value) => (
                1,
                &value.activation_id,
                &value.selected_plan_id,
                value.limits,
            ),
            PlannedActivationEntry::Scan(value) => (
                2,
                &value.activation_id,
                &value.selected_plan_id,
                value.limits,
            ),
        };
        canonical.push(tag);
        canonical.extend_from_slice(id.as_bytes());
        canonical.push(0);
        canonical.extend_from_slice(selected.as_str().as_bytes());
        canonical.extend_from_slice(&limits.maximum_active.to_le_bytes());
        canonical.extend_from_slice(&limits.maximum_queue_items.to_le_bytes());
        canonical.extend_from_slice(&limits.maximum_queue_bytes.to_le_bytes());
        canonical.extend_from_slice(&limits.maximum_items.to_le_bytes());
    }
    conduit_core::semantic_digest("conduit/lowered-fragment-activations@1", &canonical)
}
