//! Existing canonical Plan verification and its bounded preparation descriptor.
use super::*;
pub fn verify_plan(plan: &Plan) -> bool {
    verify_plan_at_depth(plan, 0)
}

pub(crate) fn verify_plan_at_depth(plan: &Plan, depth: u8) -> bool {
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
    commitments.sort_unstable();
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
        && plan.fragments.iter().all(verify_plan_fragment_contents)
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

fn verify_plan_fore_lines(plan: &Plan) -> bool {
    verify_fore_line_capacity(
        plan.fragments
            .iter()
            .flat_map(|fragment| fragment.fore_ports.iter()),
    )
}

pub(super) fn verify_fore_line_capacity<'a, I>(fores: I) -> bool
where
    I: Iterator<Item = &'a PlannedForePort> + Clone,
{
    fores.clone().all(|fore| {
        let Some(line) = &fore.selected_line else {
            return true;
        };
        let shared = fores.clone().filter(|other| {
            other
                .selected_line
                .as_ref()
                .is_some_and(|candidate| candidate.line_id == line.line_id)
        });
        let (items, bytes, same_identity) = shared.fold((0_u32, 0_u64, true), |acc, other| {
            (
                acc.0 + u32::from(other.item_capacity),
                acc.1 + u64::from(other.byte_capacity),
                acc.2 && other.selected_line.as_ref() == Some(line),
            )
        });
        same_identity
            && items <= u32::from(line.binding.limits.maximum_in_flight_items)
            && bytes <= u64::from(line.binding.limits.maximum_buffered_bytes)
    })
}

fn verify_plan_shared_pools(plan: &Plan) -> bool {
    let Some(first) = plan.fragments.first() else {
        return true;
    };
    if first.shared_pools.is_empty() {
        return plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.placements)
            .all(|placement| placement.pool_references.is_empty());
    }
    let count = plan
        .fragments
        .iter()
        .map(|fragment| fragment.placements.len())
        .sum();
    let mut placements = Vec::with_capacity(count);
    placements.extend(
        plan.fragments
            .iter()
            .flat_map(|fragment| &fragment.placements),
    );
    for pool in &first.shared_pools {
        if pool.consumers.iter().any(|consumer| {
            placements
                .iter()
                .filter(|item| &item.placement_id == consumer)
                .count()
                != 1
        }) {
            return false;
        }
        for realization in &pool.realization_envelope {
            for line in &realization.admitted_lines {
                let binding = &line.binding;
                let peer = if binding.source.host_id == realization.host_id
                    && binding.source.boot_id == realization.boot_id
                {
                    (&binding.sink.host_id, &binding.sink.boot_id)
                } else if binding.sink.host_id == realization.host_id
                    && binding.sink.boot_id == realization.boot_id
                {
                    (&binding.source.host_id, &binding.source.boot_id)
                } else {
                    return false;
                };
                if !pool.consumers.iter().any(|consumer| {
                    placements.iter().any(|placement| {
                        &placement.placement_id == consumer
                            && &placement.host_id == peer.0
                            && &placement.boot_id == peer.1
                    })
                }) {
                    return false;
                }
            }
            if pool.member_sessions_required {
                for consumer in &pool.consumers {
                    let Some(placement) = placements
                        .iter()
                        .find(|placement| &placement.placement_id == consumer)
                    else {
                        return false;
                    };
                    if placement.host_id == realization.host_id
                        && placement.boot_id == realization.boot_id
                    {
                        continue;
                    }
                    let request = realization.admitted_lines.iter().any(|line| {
                        line.binding.source.host_id == placement.host_id
                            && line.binding.source.boot_id == placement.boot_id
                            && line.binding.sink.host_id == realization.host_id
                            && line.binding.sink.boot_id == realization.boot_id
                    });
                    let result = realization.admitted_lines.iter().any(|line| {
                        line.binding.source.host_id == realization.host_id
                            && line.binding.source.boot_id == realization.boot_id
                            && line.binding.sink.host_id == placement.host_id
                            && line.binding.sink.boot_id == placement.boot_id
                    });
                    if (!pool.member_front.inputs().is_empty() && !request)
                        || (!pool.member_front.outputs().is_empty() && !result)
                    {
                        return false;
                    }
                }
            }
        }
    }
    placements.iter().all(|placement| {
        placement.pool_references.iter().all(|reference| {
            first.shared_pools.iter().any(|pool| {
                &pool.pool_id == reference && pool.consumers.contains(&placement.placement_id)
            })
        }) && first.shared_pools.iter().all(|pool| {
            pool.consumers.contains(&placement.placement_id)
                == placement.pool_references.contains(&pool.pool_id)
        })
    })
}

pub fn verify_plan_fragment(fragment: &PlanFragment) -> bool {
    if !verify_plan_fragment_contents(fragment) {
        return false;
    }
    let mut commitments = fragment.plan_fragments.clone();
    commitments.sort_unstable();
    compute_plan_id(
        &PlotIdentity {
            source_document_id: fragment.source_document_id.clone(),
            checked_plot_id: fragment.checked_plot_id.clone(),
            expanded_plot_id: fragment.expanded_plot_id.clone(),
        },
        &fragment.realization_backs,
        &[],
        &commitments,
        &[],
    ) == fragment.plan_id
}

fn verify_plan_fragment_contents(fragment: &PlanFragment) -> bool {
    if compute_fragment_id(fragment) != fragment.fragment_id {
        return false;
    }
    let mut commitments = fragment.plan_fragments.clone();
    commitments.sort_unstable();
    if commitments != fragment.plan_fragments
        || commitments
            .windows(2)
            .any(|pair| pair[0].host_id == pair[1].host_id)
    {
        return false;
    }
    let own_matches = commitments
        .iter()
        .filter(|item| item.host_id == fragment.host_id && item.fragment_id == fragment.fragment_id)
        .count();
    own_matches == 1
        && verify_fragment_fore_ports(fragment)
        && fragment.placements.iter().all(|placement| {
            placement.terminal_transductions.iter().all(|profile| {
                capability_offer::validate_terminal_transduction_ports(
                    &placement.inputs,
                    &placement.outputs,
                    profile,
                )
                .is_ok()
            })
        })
        && state_delay::verify_fragment_state(fragment)
        && execution::verify_execution_regions(fragment)
        && execution_fusion::verify(fragment)
}

mod backing;
pub use backing::local_plan_verification_backing_bytes;
