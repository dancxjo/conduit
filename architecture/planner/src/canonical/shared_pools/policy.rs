//! Bounded Planner-supplied policy choices, outside authored Plot meaning.
use super::*;

pub const MAXIMUM_SHARED_POOL_POLICY_SELECTIONS: usize = 32;
#[derive(Debug, Copy, Clone)]
pub struct SharedPoolPolicySelection<'a> {
    pub pool_id: &'a SharedPoolId,
    pub policy: SharedPoolSelectionPolicy,
}
#[derive(Debug)]
pub enum SharedPoolPolicyPlanningError {
    InputLimit,
    InvalidIdentity,
    PoolOutsidePlot,
    DuplicatePool,
    CompletionEnvelopeLimit,
    Planning(PlannerError),
}

/// Build ordinary admitted envelopes and seal each requested policy in the Plan.
/// Unspecified pools retain the capacity reference. A completion policy requires
/// the bounded prepared-cost runtime handoff; it grants no resource or authority.
#[allow(clippy::too_many_arguments)]
pub fn plan_expanded_canonical_with_shared_pool_policies(
    plot: &ExpandedCanonicalPlot,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    bases: &[BaseImplementationId],
    options: PlanningOptions<'_>,
    requirements: &BTreeMap<SharedPoolId, SharedPoolPlanningRequirement>,
    policies: &[SharedPoolPolicySelection<'_>],
) -> Result<Plan, SharedPoolPolicyPlanningError> {
    use SharedPoolPolicyPlanningError as E;
    if policies.len() > MAXIMUM_SHARED_POOL_POLICY_SELECTIONS {
        return Err(E::InputLimit);
    }
    for (index, requested) in policies.iter().enumerate() {
        if requested.pool_id.as_str().is_empty() || requested.pool_id.as_str().len() > 256 {
            return Err(E::InvalidIdentity);
        }
        if policies[..index]
            .iter()
            .any(|p| p.pool_id == requested.pool_id)
        {
            return Err(E::DuplicatePool);
        }
        if !plot
            .shared_pools
            .iter()
            .any(|p| &p.pool_id == requested.pool_id)
        {
            return Err(E::PoolOutsidePlot);
        }
    }
    let plan = plan_shared_pools_inner(
        plot,
        hosts,
        placements,
        bases,
        options,
        requirements,
        policies,
    )
    .map_err(E::Planning)?;
    if plan.fragments.first().is_some_and(|fragment| {
        fragment.shared_pools.iter().any(|p| {
            p.selection_policy == SharedPoolSelectionPolicy::MinimumPredictedCompletionThenCapacity
                && p.realization_envelope.len() > conduit_core::MAXIMUM_PREPARED_POOL_COSTS
        })
    }) {
        return Err(E::CompletionEnvelopeLimit);
    }
    Ok(plan)
}
