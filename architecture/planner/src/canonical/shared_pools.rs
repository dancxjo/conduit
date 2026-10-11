//! Ordinary shared-pool envelopes, resource accounting and exact admission.
use super::*;
use conduit_core::{
    AuthorityGrant, PlannedSharedPool, PoolMemberLimits, PoolRealizationEnvelope, ResourceBinding,
    SharedPoolId, SharedPoolSelectionPolicy,
};
mod admission;
use admission::{pool_member_lines, validate_pool_authority};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedPoolPlanningRequirement {
    pub member_limits: PoolMemberLimits,
    pub admission_authority: AuthorityGrant,
    pub member_sessions_required: bool,
}

pub fn plan_expanded_canonical_with_shared_pools(
    plot: &ExpandedCanonicalPlot,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    bases: &[BaseImplementationId],
    options: PlanningOptions<'_>,
    requirements: &BTreeMap<SharedPoolId, SharedPoolPlanningRequirement>,
) -> Result<Plan, PlannerError> {
    let mut plan = plan_expanded_canonical_with_options(plot, hosts, placements, bases, options)?;
    if plot.shared_pools.len() != requirements.len() {
        return Err(PlannerError::InvalidSharedPool(
            "every expanded shared pool requires one exact planning requirement".into(),
        ));
    }

    let mut remaining_resources = hosts
        .iter()
        .flat_map(|host| {
            host.resources.iter().map(|resource| {
                (
                    (host.host_id.clone(), resource.pool_id.clone()),
                    resource.capacity_units,
                )
            })
        })
        .collect::<BTreeMap<_, _>>();
    for placement in plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
    {
        for resource in &placement.resources {
            let available = remaining_resources
                .get_mut(&(placement.host_id.clone(), resource.pool_id.clone()))
                .ok_or_else(|| {
                    PlannerError::InvalidSharedPool(format!(
                        "planned resource '{}' is absent from pool accounting",
                        resource.pool_id.as_str()
                    ))
                })?;
            *available = available.checked_sub(resource.units).ok_or_else(|| {
                PlannerError::InvalidSharedPool(format!(
                    "planned resource '{}' exceeds its advertised capacity",
                    resource.pool_id.as_str()
                ))
            })?;
        }
    }

    let placement_lookup = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .map(|placement| (placement.gear_id.clone(), placement.placement_id.clone()))
        .collect::<BTreeMap<_, _>>();
    let planned_gears = plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.placements)
        .collect::<Vec<_>>();
    let mut planned_pools = Vec::with_capacity(plot.shared_pools.len());
    for pool in &plot.shared_pools {
        let requirement = requirements.get(&pool.pool_id).ok_or_else(|| {
            PlannerError::InvalidSharedPool(format!(
                "shared pool '{}' has no planning requirement",
                pool.pool_id.as_str()
            ))
        })?;
        validate_pool_authority(&requirement.admission_authority, hosts)?;
        if !requirement.member_limits.is_finite_and_nonzero() {
            return Err(PlannerError::InvalidSharedPool(format!(
                "shared pool '{}' has invalid per-member bounds",
                pool.pool_id.as_str()
            )));
        }
        let mut candidates = hosts
            .iter()
            .flat_map(|host| {
                host.capabilities
                    .iter()
                    .filter(|capability| capability.checked_front() == pool.member_front)
                    .map(move |capability| (host, capability))
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|(left_host, left), (right_host, right)| {
            (&left_host.host_id, &left.capability_id)
                .cmp(&(&right_host.host_id, &right.capability_id))
        });

        let mut realization_envelope = Vec::new();
        let mut needed = pool.maximum_members;
        for (host, capability) in candidates {
            if needed == 0 {
                break;
            }
            if capability.limits.max_queue_items < requirement.member_limits.queue_item_capacity
                || capability.limits.max_queue_bytes < requirement.member_limits.queue_byte_capacity
            {
                continue;
            }
            let mut member_capacity = capability.limits.max_active_instances.min(needed);
            let mut matched_resources = Vec::new();
            for resource in &capability.resource_requirements {
                if resource.units == 0
                    || resource.protected_role.is_some()
                    || resource.content.is_some()
                {
                    return Err(PlannerError::InvalidSharedPool(format!(
                        "dynamic member capability '{}' has an unsupported resource requirement",
                        capability.capability_id.as_str()
                    )));
                }
                let matching = host
                    .resources
                    .iter()
                    .filter(|offer| offer.class_id == resource.class_id)
                    .collect::<Vec<_>>();
                if matching.len() != 1 || matching[0].content.is_some() {
                    return Err(PlannerError::InvalidSharedPool(format!(
                        "dynamic member capability '{}' requires one unambiguous non-content resource pool",
                        capability.capability_id.as_str()
                    )));
                }
                let offer = matching[0];
                let available = remaining_resources
                    .get(&(host.host_id.clone(), offer.pool_id.clone()))
                    .copied()
                    .unwrap_or(0);
                member_capacity = member_capacity.min((available / resource.units) as u16);
                matched_resources.push((resource, offer, available));
            }
            if member_capacity == 0 {
                continue;
            }
            let mut resources = Vec::with_capacity(matched_resources.len());
            for (requirement, offer, available) in matched_resources {
                let compute = requirement
                    .compute
                    .as_ref()
                    .map(|_| {
                        conduit_core::compute_reservation(
                            requirement,
                            offer,
                            available / u32::from(member_capacity),
                        )
                        .ok_or_else(|| {
                            PlannerError::InvalidSharedPool(format!(
                            "dynamic member capability '{}' has an unsatisfied compute contract",
                            capability.capability_id.as_str()
                        ))
                        })
                    })
                    .transpose()?;
                resources.push(ResourceBinding {
                    content: None,
                    pool_id: offer.pool_id.clone(),
                    class_id: offer.class_id.clone(),
                    units: compute
                        .as_ref()
                        .map_or(requirement.units, |reservation| reservation.selected_lanes),
                    protected: None,
                    compute,
                });
            }
            for resource in &resources {
                let reserved = resource
                    .units
                    .checked_mul(u32::from(member_capacity))
                    .ok_or_else(|| {
                        PlannerError::InvalidSharedPool(
                            "dynamic member resource reservation overflowed".into(),
                        )
                    })?;
                let available = remaining_resources
                    .get_mut(&(host.host_id.clone(), resource.pool_id.clone()))
                    .expect("resolved pool remains in accounting");
                *available -= reserved;
            }
            let consumers = pool
                .consumers
                .iter()
                .map(|gear| {
                    let placement_id = placement_lookup.get(gear).ok_or_else(|| {
                        PlannerError::InvalidSharedPool(format!(
                            "shared pool consumer '{}' has no exact placement",
                            gear.as_str()
                        ))
                    })?;
                    planned_gears
                        .iter()
                        .copied()
                        .find(|placement| &placement.placement_id == placement_id)
                        .ok_or_else(|| {
                            PlannerError::InvalidSharedPool(
                                "shared pool consumer placement disappeared".into(),
                            )
                        })
                })
                .collect::<Result<Vec<_>, _>>()?;
            let admitted_lines = pool_member_lines(
                host,
                &pool.member_front,
                &consumers,
                bases,
                options,
                requirement,
            )?;
            realization_envelope.push(PoolRealizationEnvelope {
                host_id: host.host_id.clone(),
                boot_id: host.boot_id.clone(),
                offer_generation: host.offer_generation,
                capability_id: capability.capability_id.clone(),
                implementation_id: capability.implementation.implementation_id.clone(),
                artifact_id: capability.implementation.artifact_id.clone(),
                member_capacity,
                resources,
                admitted_lines,
            });
            needed -= member_capacity;
        }
        if needed != 0 {
            return Err(PlannerError::InvalidSharedPool(format!(
                "shared pool '{}' lacks capacity for {} members",
                pool.pool_id.as_str(),
                pool.maximum_members
            )));
        }
        let consumers = pool
            .consumers
            .iter()
            .map(|gear| {
                placement_lookup.get(gear).cloned().ok_or_else(|| {
                    PlannerError::InvalidSharedPool(format!(
                        "shared pool consumer '{}' has no exact placement",
                        gear.as_str()
                    ))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let planned = PlannedSharedPool {
            pool_id: pool.pool_id.clone(),
            declaration_id: pool.declaration_id.clone(),
            member_front: pool.member_front.clone(),
            maximum_members: pool.maximum_members,
            member_limits: requirement.member_limits,
            member_sessions_required: requirement.member_sessions_required,
            realization_envelope,
            selection_policy:
                SharedPoolSelectionPolicy::MoreUnreservedThenLessUtilizedThenPlanOrder,
            admission_authority: requirement.admission_authority.grant_id.clone(),
            consumers,
        };
        planned.validate().map_err(|error| {
            PlannerError::InvalidSharedPool(format!(
                "shared pool '{}' is invalid: {error:?}",
                pool.pool_id.as_str()
            ))
        })?;
        planned_pools.push(planned);
    }
    for pool in &planned_pools {
        if !pool.member_sessions_required {
            continue;
        }
        for realization in &pool.realization_envelope {
            if plan.fragments.iter().any(|fragment| {
                fragment.host_id == realization.host_id && fragment.boot_id == realization.boot_id
            }) {
                continue;
            }
            let host = hosts
                .iter()
                .find(|host| {
                    host.host_id == realization.host_id
                        && host.boot_id == realization.boot_id
                        && host.offer_generation == realization.offer_generation
                })
                .ok_or_else(|| {
                    PlannerError::InvalidSharedPool(
                        "shared pool realization Host disappeared before fragment sealing".into(),
                    )
                })?;
            let expected_sign = vec![
                conduit_core::ExpectedSign::PlanFragmentReceived,
                conduit_core::ExpectedSign::PlanTerminal,
            ];
            let sign_storage_budget =
                conduit_core::mandatory_sign_storage_requirement(&expected_sign).ok_or_else(
                    || PlannerError::SignBudgetOverflow("shared pool participant fragment".into()),
                )?;
            plan.fragments.push(conduit_core::PlanFragment {
                plan_id: conduit_core::PlanId::from(""),
                fragment_id: conduit_core::FragmentId::from(""),
                source_document_id: plot.source_document_id.clone(),
                checked_plot_id: plot.checked_plot_id.clone(),
                expanded_plot_id: plot.expanded_plot_id.clone(),
                completion_policy: crate::plan_completion_policy(plot.completion),
                realization_backs: plot.realization_backs.clone(),
                host_id: host.host_id.clone(),
                boot_id: host.boot_id.clone(),
                offer_generation: host.offer_generation,
                placements: Vec::new(),
                execution_regions: Vec::new(),
                execution_fusions: Vec::new(),
                states: Vec::new(),
                connections: Vec::new(),
                fore_ports: Vec::new(),
                shared_pools: Vec::new(),
                startup_dependencies: Vec::new(),
                startup_order: Vec::new(),
                cancellation_policy:
                    conduit_core::CancellationPolicy::CancelAllAndRejectLateCompletion,
                terminal_policy: conduit_core::TerminalPolicy::RequireAllPlacementsAndConnections,
                expected_terminals: vec![conduit_core::ExpectedTerminal::PlanCompleted],
                expected_sign,
                sign_storage_budget,
                plan_fragments: Vec::new(),
            });
        }
    }
    plan.fragments.sort_by(|left, right| {
        (&left.host_id, &left.boot_id).cmp(&(&right.host_id, &right.boot_id))
    });
    for fragment in &mut plan.fragments {
        fragment.shared_pools = planned_pools.clone();
    }
    Ok(conduit_core::seal_plan_with_completion(
        PlotIdentity {
            source_document_id: plot.source_document_id.clone(),
            checked_plot_id: plot.checked_plot_id.clone(),
            expanded_plot_id: plot.expanded_plot_id.clone(),
        },
        crate::plan_completion_policy(plot.completion),
        plan.fragments,
    ))
}
