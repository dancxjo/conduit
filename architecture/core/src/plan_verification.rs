//! Shared rich/streamed fingerprint gate followed by identical structural checks.
use super::*;
pub(super) fn verify_at_depth(
    plan: &Plan,
    depth: u8,
    fingerprint: &mut impl FnMut(&PlanFragment) -> Option<FragmentId>,
) -> bool {
    // Supported rich flat Plans use the same predicates as immutable borrowed
    // inputs. Preserve the supplied streamed-configuration fingerprint callback.
    if let Ok(view) = FlatPlanRef::from_plan(plan) {
        if view.profile().is_ok() {
            return view.root_fingerprint_matches()
                && fingerprint(&plan.fragments[0])
                    .is_some_and(|id| id.as_str() == view.fragment_id)
                && view.structure();
        }
    }
    let plot_identity = PlotIdentity {
        source_document_id: plan.source_document_id.clone(),
        checked_plot_id: plan.checked_plot_id.clone(),
        expanded_plot_id: plan.expanded_plot_id.clone(),
    };
    let mut commitments = plan
        .fragments
        .iter()
        .map(|fragment| FragmentCommitment {
            host_id: fragment.host_id.clone(),
            fragment_id: fragment.fragment_id.clone(),
        })
        .collect::<Vec<_>>();
    commitments.sort();
    plan.realization_backs
        .windows(2)
        .all(|pair| pair[0] < pair[1])
        && plan.realization_backs.iter().all(|back| {
            !back.invocation_path.is_empty()
                && !back.kind_id.as_str().is_empty()
                && !back.kind_contract_revision.as_str().is_empty()
                && !back.source_document_id.as_str().is_empty()
                && !back.checked_plot_id.as_str().is_empty()
        })
        && plan.plan_id
            == compute_plan_id(
                &plot_identity,
                &plan.realization_backs,
                &plan.activations,
                &commitments,
                &plan.activation_preparations,
            )
        && plan.fragments.iter().all(|fragment| {
            fingerprint(fragment)
                .is_some_and(|id| verify_plan_fragment_contents_with_fingerprint(fragment, &id))
        })
        && plan.fragments.iter().all(|fragment| {
            fragment.plan_id == plan.plan_id
                && fragment.source_document_id == plan.source_document_id
                && fragment.checked_plot_id == plan.checked_plot_id
                && fragment.expanded_plot_id == plan.expanded_plot_id
                && fragment.completion_policy == plan.completion_policy
                && fragment.realization_backs == plan.realization_backs
        })
        && plan
            .fragments
            .first()
            .is_none_or(|first| first.plan_fragments.len() == plan.fragments.len())
        && plan.fragments.first().is_none_or(|first| {
            first
                .shared_pools
                .iter()
                .all(|pool| pool.validate().is_ok())
                && plan
                    .fragments
                    .iter()
                    .all(|fragment| fragment.shared_pools == first.shared_pools)
        })
        && state_delay::verify_plan_states(plan)
        && verify_plan_shared_pools(plan)
        && verify_plan_fore_lines(plan)
        && verify_plan_connections(plan)
        && planned_activation::verify_planned_activations(plan, depth)
        && planned_activation::verify_activation_preparations(plan)
}
