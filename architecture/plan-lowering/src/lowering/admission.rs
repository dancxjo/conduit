//! Common admission checks before allocating numeric lowering tables.
use super::{KernelStorageProfile, LoweringError};
use conduit_core::{mandatory_sign_storage_requirement_ref, verify_plan_fragment, PlanFragment};

pub(super) fn validate_fragment(
    fragment: &PlanFragment,
    profile: KernelStorageProfile,
) -> Result<(), LoweringError> {
    if !verify_plan_fragment(fragment) {
        return Err(LoweringError::InvalidFragment);
    }
    validate_verified_fragment(fragment, profile)
}

/// The caller verified the complete Plan, including activation commitments.
/// Fragment-only verification cannot reconstruct an activation-bearing PlanId.
pub(crate) fn validate_verified_fragment(
    fragment: &PlanFragment,
    profile: KernelStorageProfile,
) -> Result<(), LoweringError> {
    validate_verified_fragment_facts(
        fragment.into(),
        conduit_core::PlannedGearRefs::Owned(&fragment.placements),
        fragment.into(),
        profile,
    )
}

pub(crate) fn validate_verified_fragment_facts(
    header: conduit_core::FragmentFingerprintHeaderRef<'_>,
    placements: conduit_core::PlannedGearRefs<'_>,
    tail: conduit_core::FragmentFingerprintTailRef<'_>,
    profile: KernelStorageProfile,
) -> Result<(), LoweringError> {
    // No current lowering profile installs native resumable continuations.
    if let Some(region) = header.execution_regions.iter().find(|region| {
        matches!(
            region.scheduling,
            conduit_core::ExecutionScheduling::ProtectedPreemptibleStep { .. }
        )
    }) {
        return Err(LoweringError::UnsupportedExecutionRegion(
            region.region_id.clone(),
        ));
    }
    if !profile.supports_partitioned_regions() {
        if let Some(region) = header
            .execution_regions
            .iter()
            .find(|r| r.scheduling.is_partitioned())
        {
            return Err(LoweringError::UnsupportedExecutionRegion(
                region.region_id.clone(),
            ));
        }
    }
    // Fresh initialization profiles must never silently reset retained State.
    if let Some(state) = tail.states.iter().find(|state| state.retained.is_some()) {
        if !profile.supports_owned_state_continuity() {
            return Err(LoweringError::UnsupportedState(state.state_id.clone()));
        }
    }
    if let Some(state) = tail.states.first() {
        let Some((instances, bytes)) = profile.state_storage() else {
            return Err(LoweringError::UnsupportedState(state.state_id.clone()));
        };
        if tail.states.len() > usize::from(instances)
            || tail
                .states
                .iter()
                .any(|state| state.maximum_value_bytes > bytes)
        {
            return Err(LoweringError::StateStorageExceeded);
        }
    }
    // A host admitted only as a dynamic shared-pool realization still owns a
    // real Plan fragment and fixed pool/session state even though it has no
    // statically placed Gear yet.
    if placements.is_empty() && tail.shared_pools.is_empty() {
        return Err(LoweringError::EmptyFragment);
    }
    let mut expected = mandatory_sign_storage_requirement_ref(tail.expected_sign)
        .ok_or(LoweringError::SignBudgetInvalid)?;
    let state = conduit_core::state_resource_budget(tail.states)
        .map_err(|_| LoweringError::SignBudgetInvalid)?
        .sign_storage;
    expected.item_capacity = expected
        .item_capacity
        .checked_add(state.item_capacity)
        .ok_or(LoweringError::SignBudgetInvalid)?;
    expected.byte_capacity = expected
        .byte_capacity
        .checked_add(state.byte_capacity)
        .ok_or(LoweringError::SignBudgetInvalid)?;
    if expected != *tail.sign_storage_budget {
        return Err(LoweringError::SignBudgetInvalid);
    }
    Ok(())
}
