use crate::prelude::*;
use crate::{
    default_placements_unvalidated, plan_validated_form,
    plan_validated_form_with_connection_limits, ConnectionEndpoints, ConnectionQueueLimits,
    PlacementChoices, PlannerError, PlanningOptions,
};
use alloc::collections::{BTreeMap, BTreeSet};
use conduit_core::{
    AdmittedLine, AuthorityGrant, BaseImplementationId, FormIdentity, HostAdvertisement,
    LineAvailability, Plan, PlannedGear, PlannedSharedPool, PoolMemberLimits,
    PoolRealizationEnvelope, ResourceBinding, SharedPoolId, SharedPoolSelectionPolicy,
    DEFAULT_CONNECTION_BYTE_CAPACITY, DEFAULT_CONNECTION_ITEM_CAPACITY,
    SHARED_POOL_ADMIT_AUTHORITY_CONTRACT, SHARED_POOL_ADMIT_HOST_OPERATION_CONTRACT,
    SHARED_POOL_AUTHORITY_SUBJECT_KIND,
};
use conduit_form::{
    expand_canonical_form, expand_canonical_form_with_backs, CanonicalBackCatalog, CheckedForm,
    CheckedSyntaxDocument, ExpandedCanonicalForm, ProfileCatalog,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanonicalRealizationMode {
    Direct,
    RecursiveBack,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedCanonicalRealization {
    pub mode: CanonicalRealizationMode,
    pub expanded: ExpandedCanonicalForm,
    pub placements: PlacementChoices,
    pub plan: Plan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalRealizationSelectionError {
    InvalidDirectExpansion(String),
    InvalidRecursiveExpansion {
        direct: PlannerError,
        recursive: String,
    },
    NoRealizablePath {
        direct: PlannerError,
        recursive: PlannerError,
    },
}

/// Plans between peer realizations of one checked caller without changing its
/// source. A fully admitted direct Plan wins; otherwise the same checked
/// caller is expanded through the admitted exact Back catalog and planned
/// normally. Mere capability availability cannot suppress a valid Back.
#[allow(clippy::too_many_arguments)]
pub fn plan_canonical_realization_with_options(
    document: &CheckedSyntaxDocument,
    form_name: &str,
    catalog: &ProfileCatalog,
    backs: &CanonicalBackCatalog,
    hosts: &[HostAdvertisement],
    bases: &[BaseImplementationId],
    options: PlanningOptions<'_>,
) -> Result<PlannedCanonicalRealization, CanonicalRealizationSelectionError> {
    let direct = expand_canonical_form(document, form_name, catalog).map_err(|error| {
        CanonicalRealizationSelectionError::InvalidDirectExpansion(error.to_string())
    })?;
    match plan_default_candidate(direct, hosts, bases, options) {
        Ok((expanded, placements, plan)) => Ok(PlannedCanonicalRealization {
            mode: CanonicalRealizationMode::Direct,
            expanded,
            placements,
            plan,
        }),
        Err(direct_error) => {
            let recursive = expand_canonical_form_with_backs(document, form_name, catalog, backs)
                .map_err(|error| {
                CanonicalRealizationSelectionError::InvalidRecursiveExpansion {
                    direct: direct_error.clone(),
                    recursive: error.to_string(),
                }
            })?;
            let (expanded, placements, plan) =
                plan_default_candidate(recursive, hosts, bases, options).map_err(
                    |recursive_error| CanonicalRealizationSelectionError::NoRealizablePath {
                        direct: direct_error,
                        recursive: recursive_error,
                    },
                )?;
            Ok(PlannedCanonicalRealization {
                mode: CanonicalRealizationMode::RecursiveBack,
                expanded,
                placements,
                plan,
            })
        }
    }
}

fn plan_default_candidate(
    expanded: ExpandedCanonicalForm,
    hosts: &[HostAdvertisement],
    bases: &[BaseImplementationId],
    options: PlanningOptions<'_>,
) -> Result<(ExpandedCanonicalForm, PlacementChoices, Plan), PlannerError> {
    let placements = default_expanded_placements(&expanded, hosts)?;
    let plan = plan_expanded_canonical_with_options(&expanded, hosts, &placements, bases, options)?;
    Ok((expanded, placements, plan))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedPoolPlanningRequirement {
    pub member_limits: PoolMemberLimits,
    pub admission_authority: AuthorityGrant,
    pub member_sessions_required: bool,
}

pub fn default_expanded_placements(
    form: &ExpandedCanonicalForm,
    hosts: &[HostAdvertisement],
) -> Result<PlacementChoices, PlannerError> {
    form.validate_expansion()
        .map_err(|error| PlannerError::InvalidFormIdentity(error.to_string()))?;
    default_placements_unvalidated(&form.gears, hosts)
}

pub fn plan_expanded_canonical(
    form: &ExpandedCanonicalForm,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    bases: &[BaseImplementationId],
) -> Result<Plan, PlannerError> {
    plan_expanded_canonical_with_options(
        form,
        hosts,
        placements,
        bases,
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: DEFAULT_CONNECTION_ITEM_CAPACITY,
            connection_byte_capacity: DEFAULT_CONNECTION_BYTE_CAPACITY,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
}

pub fn plan_expanded_canonical_with_options(
    form: &ExpandedCanonicalForm,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    bases: &[BaseImplementationId],
    options: PlanningOptions<'_>,
) -> Result<Plan, PlannerError> {
    form.validate_expansion()
        .map_err(|error| PlannerError::InvalidFormIdentity(error.to_string()))?;
    let planning_form = CheckedForm {
        source_document_id: form.source_document_id.clone(),
        checked_form_id: form.checked_form_id.clone(),
        expanded_form_id: form.expanded_form_id.clone(),
        name: form.name.clone(),
        completion: form.completion,
        gears: form.gears.clone(),
        connections: form.connections.clone(),
        exports: Vec::new(),
        nested_forms: Vec::new(),
    };
    let plan = plan_validated_form(&planning_form, hosts, placements, bases, options)?;
    Ok(
        conduit_core::seal_plan_with_realization_backs_and_completion(
            conduit_core::FormIdentity {
                source_document_id: form.source_document_id.clone(),
                checked_form_id: form.checked_form_id.clone(),
                expanded_form_id: form.expanded_form_id.clone(),
            },
            crate::plan_completion_policy(form.completion),
            form.realization_backs.clone(),
            plan.fragments,
        ),
    )
}

pub fn plan_expanded_canonical_with_connection_limits(
    form: &ExpandedCanonicalForm,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    bases: &[BaseImplementationId],
    options: PlanningOptions<'_>,
    connection_limits: &BTreeMap<ConnectionEndpoints, ConnectionQueueLimits>,
) -> Result<Plan, PlannerError> {
    form.validate_expansion()
        .map_err(|error| PlannerError::InvalidFormIdentity(error.to_string()))?;
    let planning_form = CheckedForm {
        source_document_id: form.source_document_id.clone(),
        checked_form_id: form.checked_form_id.clone(),
        expanded_form_id: form.expanded_form_id.clone(),
        name: form.name.clone(),
        completion: form.completion,
        gears: form.gears.clone(),
        connections: form.connections.clone(),
        exports: Vec::new(),
        nested_forms: Vec::new(),
    };
    let plan = plan_validated_form_with_connection_limits(
        &planning_form,
        hosts,
        placements,
        bases,
        options,
        connection_limits,
    )?;
    Ok(
        conduit_core::seal_plan_with_realization_backs_and_completion(
            conduit_core::FormIdentity {
                source_document_id: form.source_document_id.clone(),
                checked_form_id: form.checked_form_id.clone(),
                expanded_form_id: form.expanded_form_id.clone(),
            },
            crate::plan_completion_policy(form.completion),
            form.realization_backs.clone(),
            plan.fragments,
        ),
    )
}

pub fn plan_expanded_canonical_with_shared_pools(
    form: &ExpandedCanonicalForm,
    hosts: &[HostAdvertisement],
    placements: &PlacementChoices,
    bases: &[BaseImplementationId],
    options: PlanningOptions<'_>,
    requirements: &BTreeMap<SharedPoolId, SharedPoolPlanningRequirement>,
) -> Result<Plan, PlannerError> {
    let mut plan = plan_expanded_canonical_with_options(form, hosts, placements, bases, options)?;
    if form.shared_pools.len() != requirements.len() {
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
    let mut planned_pools = Vec::with_capacity(form.shared_pools.len());
    for pool in &form.shared_pools {
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
    for fragment in &mut plan.fragments {
        fragment.shared_pools = planned_pools.clone();
    }
    Ok(conduit_core::seal_plan_with_completion(
        FormIdentity {
            source_document_id: form.source_document_id.clone(),
            checked_form_id: form.checked_form_id.clone(),
            expanded_form_id: form.expanded_form_id.clone(),
        },
        crate::plan_completion_policy(form.completion),
        plan.fragments,
    ))
}

fn validate_pool_authority(
    grant: &AuthorityGrant,
    hosts: &[HostAdvertisement],
) -> Result<(), PlannerError> {
    let exact_scope = grant.contract_id.as_str() == SHARED_POOL_ADMIT_AUTHORITY_CONTRACT
        && grant.host_operation_contract_id.as_str() == SHARED_POOL_ADMIT_HOST_OPERATION_CONTRACT
        && grant.subject_kind.as_str() == SHARED_POOL_AUTHORITY_SUBJECT_KIND
        && !grant.grant_id.as_str().is_empty()
        && hosts.iter().any(|host| {
            host.host_id == grant.host_id
                && host.boot_id == grant.boot_id
                && host
                    .capabilities
                    .iter()
                    .any(|capability| capability.capability_id == grant.capability_id)
        });
    if !exact_scope {
        return Err(PlannerError::InvalidSharedPool(
            "shared-pool admission authority is missing or has the wrong exact scope".into(),
        ));
    }
    Ok(())
}

fn pool_member_lines(
    member_host: &HostAdvertisement,
    member_front: &conduit_core::CheckedFace,
    consumers: &[&PlannedGear],
    bases: &[BaseImplementationId],
    options: PlanningOptions<'_>,
    requirement: &SharedPoolPlanningRequirement,
) -> Result<Vec<AdmittedLine>, PlannerError> {
    if !requirement.member_sessions_required {
        return Ok(Vec::new());
    }

    let mut admitted = Vec::new();
    let mut identities = BTreeSet::new();
    for consumer in consumers {
        if consumer.host_id == member_host.host_id && consumer.boot_id == member_host.boot_id {
            continue;
        }
        let mut directions = Vec::with_capacity(2);
        if !member_front.inputs().is_empty() {
            directions.push((
                &consumer.host_id,
                &consumer.boot_id,
                &member_host.host_id,
                &member_host.boot_id,
                "consumer-to-member",
            ));
        }
        if !member_front.outputs().is_empty() {
            directions.push((
                &member_host.host_id,
                &member_host.boot_id,
                &consumer.host_id,
                &consumer.boot_id,
                "member-to-consumer",
            ));
        }
        for (source_host, source_boot, sink_host, sink_boot, direction) in directions {
            let matches = options
                .line_offers
                .iter()
                .filter(|offer| {
                    offer.binding.source.host_id == *source_host
                        && offer.binding.source.boot_id == *source_boot
                        && offer.binding.sink.host_id == *sink_host
                        && offer.binding.sink.boot_id == *sink_boot
                        && bases.contains(&offer.binding.base)
                        && offer.validate_sign_identity()
                        && offer.availability.availability == LineAvailability::Ready
                        && offer.binding.limits.maximum_in_flight_items
                            >= requirement.member_limits.queue_item_capacity
                        && offer.binding.limits.maximum_payload_bytes
                            >= requirement.member_limits.queue_byte_capacity
                        && offer.binding.limits.maximum_buffered_bytes
                            >= requirement.member_limits.queue_byte_capacity
                        && offer.binding.limits.maximum_frame_bytes
                            >= offer.binding.limits.maximum_payload_bytes
                })
                .collect::<Vec<_>>();
            if matches.len() != 1 {
                return Err(PlannerError::InvalidSharedPool(format!(
                    "shared pool member session requires one exact ready bounded {direction} Line between '{}@{}' and '{}@{}'; found {}",
                    source_host.as_str(), source_boot.as_str(), sink_host.as_str(), sink_boot.as_str(), matches.len()
                )));
            }
            let line = matches[0].admitted_line();
            if !identities.insert((line.line_id.clone(), line.binding.binding_id.clone())) {
                return Err(PlannerError::InvalidSharedPool(
                    "shared pool member session repeats an admitted Line identity".into(),
                ));
            }
            admitted.push(line);
        }
    }
    Ok(admitted)
}
